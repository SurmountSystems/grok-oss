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
use crate::views::limits_honesty::NOTE_LIMITS_PRINTOUT_NOT_USAGE;
use crate::views::limits_snapshot::{
    AllowanceMeterTone, LimitsSnapshot, countdown_is_zero, earliest_reset_at,
    format_limits_detail_with_meter_source, format_reset_countdown,
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
        self.content_lines_emphasizing_meter_source(now, None)
    }

    /// Content lines with a `/limits meter` pin on the **Active:** line.
    pub fn content_lines_emphasizing_meter_source(
        &self,
        now: DateTime<Utc>,
        meter_source: Option<xai_grok_shell::auth::limits_pins::MeterSource>,
    ) -> Vec<String> {
        let mut body = format_limits_detail_with_meter_source(&self.snapshot, meter_source);
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
        tabs: Some(CARD_TABS),
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

/// Credits is the meter card. Limits is included-period pacing for the week.
const CARD_TABS: &[&str] = &["Credits", "Limits"];

/// Limits tab copy. The status row does not use this phrase.
fn linear_week_label(pacing: xai_grok_shell::token_economy::PeriodPacing) -> String {
    let points = pacing.abs_delta_rounded();
    if points == 0 {
        "on a linear week".to_string()
    } else if pacing.delta_pct > 0.0 {
        format!("{points}% ahead of a linear week")
    } else {
        format!("{points}% behind a linear week")
    }
}

fn limits_tab_lines(state: &LimitsModalState, now: DateTime<Utc>) -> Vec<String> {
    let Some(included) = state.snapshot.primary.included.as_ref() else {
        return vec!["Pacing for this week is not known yet.".to_string()];
    };
    if included.period_label != "Weekly" {
        return vec!["Pacing for this week is not known yet.".to_string()];
    }
    let Some(end) = included.next_reset_at else {
        return vec!["Pacing for this week is not known yet.".to_string()];
    };
    let Some(start) = xai_grok_shell::token_economy::resolve_period_start(
        None,
        Some(end),
        Some("USAGE_PERIOD_TYPE_WEEKLY"),
    ) else {
        return vec!["Pacing for this week is not known yet.".to_string()];
    };
    let Some(pacing) =
        xai_grok_shell::token_economy::compute_period_pacing(included.used_pct, start, end, now)
    else {
        return vec!["Pacing for this week is not known yet.".to_string()];
    };
    vec![linear_week_label(pacing)]
}

/// Credits tab field names. Console API credits are team prepaid remaining.
fn credits_tab_meter_line(raw: &str) -> String {
    let trimmed = raw.trim_start();
    let indent = &raw[..raw.len() - trimmed.len()];
    if let Some(rest) = trimmed.strip_prefix("SuperGrok dollar credits:") {
        return format!("{indent}Personal credits:{rest}");
    }
    if let Some(rest) = trimmed.strip_prefix("Team prepaid remaining:") {
        let value = rest.trim();
        if value == "team prepaid unavailable" {
            return format!("{indent}Console API credits: not available");
        }
        return format!("{indent}Console API credits: {value}");
    }
    raw.to_string()
}
/// Footer id for the other spend choice. Esc close stays id 1.
pub(crate) const SPEND_OTHER_CHOICE_ID: usize = 7;

fn console_api_credits_balance_available(state: &LimitsModalState) -> bool {
    matches!(state.snapshot.console.balance_cents, Some(cents) if cents > 0)
}

/// Console API credits are in use only when the meter pin is console and team
/// prepaid remaining is available. This is not the `use_console` flag.
fn using_console_api_credits(state: &LimitsModalState) -> bool {
    let pins = xai_grok_shell::auth::limits_pins::load_limits_pins();
    pins.meter_source == Some(xai_grok_shell::auth::limits_pins::MeterSource::Console)
        && console_api_credits_balance_available(state)
}

fn spend_status_and_button(state: &LimitsModalState) -> (&'static str, &'static str) {
    if using_console_api_credits(state) {
        ("Using credits", "Use limits")
    } else {
        ("Using limits", "Use credits")
    }
}

fn persist_other_spend_choice(state: &LimitsModalState) -> std::io::Result<()> {
    use xai_grok_shell::auth::limits_pins::{MeterSource, load_limits_pins, save_limits_pins};
    let mut pins = load_limits_pins();
    pins.use_console = false;
    if using_console_api_credits(state) {
        pins.meter_source = Some(MeterSource::Included);
    } else if console_api_credits_balance_available(state) {
        pins.meter_source = Some(MeterSource::Console);
        pins.stay_supergrok = false;
    } else if pins.meter_source == Some(MeterSource::DollarCredits) {
        pins.meter_source = Some(MeterSource::Included);
    }
    save_limits_pins(&pins)
}

/// Click on the limits card. The spend button writes the meter pin.
pub fn handle_limits_mouse(
    state: &mut LimitsModalState,
    kind: crossterm::event::MouseEventKind,
    column: u16,
    row: u16,
) -> LimitsModalOutcome {
    match modal_window::handle_modal_mouse(&mut state.window, kind, column, row) {
        ModalWindowOutcome::CloseRequested => LimitsModalOutcome::Close,
        ModalWindowOutcome::ShortcutActivated(id) if id == SPEND_OTHER_CHOICE_ID => {
            if persist_other_spend_choice(state).is_ok() {
                LimitsModalOutcome::Changed
            } else {
                LimitsModalOutcome::Unchanged
            }
        }
        ModalWindowOutcome::Unhandled => LimitsModalOutcome::Unchanged,
        _ => LimitsModalOutcome::Changed,
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
    let (status_label, button_label) = spend_status_and_button(state);
    let shortcuts = [
        Shortcut {
            label: status_label,
            clickable: false,
            id: 0,
        },
        Shortcut {
            label: button_label,
            clickable: true,
            id: SPEND_OTHER_CHOICE_ID,
        },
        Shortcut {
            label: "Esc close",
            clickable: true,
            id: 1,
        },
    ];
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
        tabs: Some(CARD_TABS),
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
    // Detect the included-allowance meter on the unwrapped line: wrap can
    // split "Included ... allowance:" from "% used" and would skip the bar.
    let width = content.width as usize;
    let primary_bar = state.snapshot.primary.included.as_ref().map(|inc| {
        let rem = inc.remaining_fraction();
        let tone = AllowanceMeterTone::from_used_pct(inc.used_pct);
        (rem, tone)
    });
    let mut display_lines: Vec<String> = Vec::new();
    let mut injected_bar = false;
    let meter_source = xai_grok_shell::auth::limits_pins::load_limits_pins().meter_source;
    let body = if state.window.active_tab == 1 {
        limits_tab_lines(state, now)
    } else {
        state.content_lines_emphasizing_meter_source(now, meter_source)
    };
    for raw in body {
        let is_allowance_meter = is_included_allowance_used_line(&raw);
        // The fail-open printout note is required in the body text (CLI /
        // agents) before percents. Full wrap at modal width is ~15 rows and
        // buries the remaining bar under the fold. TUI shows a one-sentence
        // banner; the full note stays in content_lines.
        let display_src = if raw.as_str() == NOTE_LIMITS_PRINTOUT_NOT_USAGE {
            TUI_PRINTOUT_BANNER.to_string()
        } else if state.window.active_tab == 0 {
            credits_tab_meter_line(raw.as_str())
        } else {
            raw
        };
        display_lines.extend(wrap_plain_line(&display_src, width));
        if primary_bar.is_some() && !injected_bar && is_allowance_meter {
            display_lines.push(REMAINING_BAR_SENTINEL.to_string());
            injected_bar = true;
        }
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

/// One-sentence TUI stand-in for [`NOTE_LIMITS_PRINTOUT_NOT_USAGE`].
const TUI_PRINTOUT_BANNER: &str =
    "Note: grok-oss limits is a client printout, not xAI billing truth.";

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
        || text.contains("independently polled the same included percent")
        || text.contains("Usage-page proof")
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
        let long = "Note: SuperGrok personal and SuperGrok business independently polled the same included percent and reset (client printout, not Usage-page proof they share one weekly window). Not console team prepaid.";
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
        assert!(joined.contains("weekly window"), "{joined}");
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
            joined.contains("Team prepaid remaining: no management key"),
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
        let body = "Live sampling: SuperGrok session\n\nSuperGrok:\n  Next reset: August 3, 19:25\n  SuperGrok dollar credits: $1";
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
    /// Clicking the painted limits chip opens the card. The card uses the
    /// existing tab bar: Credits and Limits. The Limits tab shows ahead of
    /// or behind a linear week. That pacing is not on the header. Opening
    /// the card does not add an HTTP call when the last leader answer is
    /// inside one minute. Changing tabs does not change the spend pin.
    #[test]
    fn clicking_the_chip_opens_the_card_and_the_limits_tab_shows_ahead_or_behind_a_linear_week() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        use crossterm::event::{MouseButton, MouseEventKind};

        use crate::actions::ActionRegistry;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::app::bundle::BundleState;
        use crate::scrollback::render::ScratchBuffer;

        struct EnvGuard {
            prev_home: Option<std::ffi::OsString>,
            prev_disable: Option<std::ffi::OsString>,
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                // Safety: this test runs alone (`--test-threads=1`) and restores both vars.
                unsafe {
                    match self.prev_home.take() {
                        Some(value) => std::env::set_var("GROK_HOME", value),
                        None => std::env::remove_var("GROK_HOME"),
                    }
                    match self.prev_disable.take() {
                        Some(value) => std::env::set_var("GROK_DISABLE_SHARED_RATE_LIMIT", value),
                        None => std::env::remove_var("GROK_DISABLE_SHARED_RATE_LIMIT"),
                    }
                }
            }
        }

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let env = EnvGuard {
            prev_home: std::env::var_os("GROK_HOME"),
            prev_disable: std::env::var_os("GROK_DISABLE_SHARED_RATE_LIMIT"),
        };
        // Safety: restored by EnvGuard. Shared snapshot coordination must stay on.
        unsafe {
            std::env::set_var("GROK_HOME", home.path());
            std::env::remove_var("GROK_DISABLE_SHARED_RATE_LIMIT");
        }

        let pins = xai_grok_shell::auth::limits_pins::LimitsPins {
            meter_source: Some(xai_grok_shell::auth::limits_pins::MeterSource::Included),
            ..Default::default()
        };
        xai_grok_shell::auth::limits_pins::save_limits_pins_under(home.path(), &pins)
            .expect("write spend pin");
        let pin_before = std::fs::read(home.path().join("limits_pins.json")).expect("read pin");

        let end = DateTime::parse_from_rfc3339("2026-08-08T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let bal = weekly_bal(62.0, end);

        crate::appearance::cache::set_hide_header(false);
        let mut agent = make_agent();
        agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
        agent.credit_balance = Some(bal.clone());

        let area = Rect::new(0, 0, 120, 40);
        let mut header = Buffer::empty(area);
        let mut scratch = ScratchBuffer::new();
        agent.draw(
            area,
            &mut header,
            &ActionRegistry::defaults(),
            &mut scratch,
            None,
            false,
            BannerSlotParams::none(),
            &BundleState::default(),
            false,
            false,
            &mut Vec::new(),
            AppRenderParams::default(),
        );
        let header_text = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .filter_map(|x| header.cell((x, y)).map(|cell| cell.symbol().to_string()))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let row = header_text
            .lines()
            .find(|line| line.contains("/tmp"))
            .unwrap_or("")
            .to_string();
        assert!(
            row.contains("limits 62%"),
            "painted chip must show included used percent:\n{row}"
        );
        let hit = agent
            .hit_credits
            .rect
            .expect("painted chip must arm a click rectangle");
        assert!(
            agent.hit_credits.contains(hit.x, hit.y),
            "the click point is the painted chip"
        );
        for token in [
            "behind linear burn",
            "15m",
            "24h",
            "business",
            "personal",
            "SuperGrok period",
            "linear week",
        ] {
            assert!(
                !row.contains(token),
                "header must not contain {token}:\n{row}"
            );
        }

        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let mut state = LimitsModalState::new(snap);
        let theme = Theme::default();
        let modal_area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(modal_area);
        render_limits_modal(&mut buf, modal_area, &mut state, &theme, false, now);
        assert_eq!(state.window.tab_count, 2, "Limits modal passes no tabs");

        let painted = |buffer: &Buffer| -> String {
            (0..modal_area.height)
                .map(|y| {
                    (0..modal_area.width)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let credits = painted(&buf);
        assert!(
            credits.contains("Credits") && credits.contains("Limits"),
            "clicking the chip must open Credits and Limits tabs:\n{credits}"
        );
        assert!(
            !credits.contains("linear week"),
            "pacing stays off the Credits tab and off the header:\n{credits}"
        );

        let limits_tab = state.window.tab_rects[1].expect("Limits tab rectangle");
        let outcome = modal_window::handle_modal_mouse(
            &mut state.window,
            MouseEventKind::Down(MouseButton::Left),
            limits_tab.x,
            limits_tab.y,
        );
        assert_eq!(outcome, ModalWindowOutcome::TabChanged(1));
        let pin_after_tab = std::fs::read(home.path().join("limits_pins.json")).expect("pin");
        assert_eq!(
            pin_before, pin_after_tab,
            "changing tabs must not change the spend pin"
        );

        let mut buf = Buffer::empty(modal_area);
        render_limits_modal(&mut buf, modal_area, &mut state, &theme, false, now);
        let limits = painted(&buf);
        assert!(
            limits.contains("12% ahead of a linear week")
                || limits.contains("behind a linear week"),
            "Limits tab must show ahead of or behind a linear week:\n{limits}"
        );
        assert!(
            limits.contains("12% ahead of a linear week"),
            "half a week at 62% used is 12% ahead of a linear week:\n{limits}"
        );
        assert!(
            !row.contains("linear week") && !row.contains("behind linear burn"),
            "that pacing is not on the header:\n{row}"
        );

        let now_ms = xai_grok_shell::auth::limits_snapshot_hub::now_unix_ms();
        let doc = xai_grok_shell::auth::LimitsSnapshotDocument::empty(now_ms);
        xai_grok_shell::auth::write_limits_snapshot_file(home.path(), &doc)
            .expect("young leader answer");
        let http = Arc::new(AtomicU32::new(0));
        let http_fetch = Arc::clone(&http);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime
            .block_on(xai_grok_shell::auth::coordinate_limits_snapshot(
                home.path(),
                xai_grok_shell::auth::LimitsSnapshotMode::ForceRefresh,
                now_ms.saturating_add(30_000),
                || {
                    let http_fetch = Arc::clone(&http_fetch);
                    async move {
                        http_fetch.fetch_add(1, Ordering::SeqCst);
                        xai_grok_shell::auth::LimitsSnapshotDocument::empty(now_ms)
                    }
                },
            ))
            .expect("open card force refresh");
        assert_eq!(
            http.load(Ordering::SeqCst),
            0,
            "opening the card must not add an HTTP call when the last leader answer is inside one minute"
        );
        drop(env);
    }
    #[test]
    fn credits_tab_shows_personal_credits_separate_from_console_api_credits_and_a_failed_fetch_is_not_a_balance()
     {
        let reset = DateTime::parse_from_rfc3339("2026-08-08T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let paint =
            |personal_cents: i64, console_cents: Option<i64>, failed_console: bool| -> String {
                let mut bal = weekly_bal(10.0, reset);
                bal.prepaid_balance_cents = Some(personal_cents);
                let snap = LimitsSnapshot::from_billing(
                    Some(&bal),
                    None,
                    SamplingIdentityKind::SuperGrokSession,
                );
                let snap = if failed_console {
                    snap.with_console_balance_cents(None)
                        .with_console_prepaid_gap(ConsoleTeamPrepaidGap::Unavailable)
                } else {
                    snap.with_console_balance_cents(console_cents)
                };
                let mut state = LimitsModalState::new(snap);
                let theme = Theme::default();
                let area = Rect::new(0, 0, 100, 40);
                let mut buf = Buffer::empty(area);
                render_limits_modal(&mut buf, area, &mut state, &theme, false, now);
                assert_eq!(state.window.active_tab, 0, "Credits is the open tab");
                (0..area.height)
                    .map(|y| {
                        (0..area.width)
                            .map(|x| buf[(x, y)].symbol())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };

        // Two fixture replies. Amounts are not taken from chat.
        let first = paint(4321, Some(8900), false);
        let second = paint(150, Some(2675), false);
        let failed = paint(4321, None, true);

        let field = |text: &str, label: &str| -> String {
            let Some(line) = text.lines().find(|line| line.contains(label)) else {
                return String::new();
            };
            let start = line.find(label).unwrap_or(0);
            let rest = &line[start..];
            let end = rest.find('│').unwrap_or(rest.len());
            rest[..end].trim().to_string()
        };
        let amount = |line: &str| -> String {
            line.split_once(':')
                .map(|(_, value)| value.trim().to_string())
                .unwrap_or_default()
        };

        let personal_a = field(&first, "Personal credits:");
        let console_a = field(&first, "Console API credits:");
        let personal_b = field(&second, "Personal credits:");
        let console_b = field(&second, "Console API credits:");
        assert!(
            personal_a.starts_with("Personal credits:"),
            "Credits tab must label personal credits exactly:\n{first}"
        );
        assert!(
            console_a.starts_with("Console API credits:"),
            "Credits tab must label console API credits exactly:\n{first}"
        );
        assert!(
            amount(&personal_a).starts_with('$') && amount(&console_a).starts_with('$'),
            "two successful replies must show dollar amounts:\n{personal_a}\n{console_a}"
        );
        assert_ne!(
            amount(&personal_a),
            amount(&console_a),
            "personal credits and console API credits must differ"
        );
        assert_ne!(
            amount(&personal_a),
            amount(&personal_b),
            "two successful personal replies must show different amounts"
        );
        assert_ne!(
            amount(&console_a),
            amount(&console_b),
            "two successful console replies must show different amounts"
        );
        assert!(
            !first.contains("Business credits") && !second.contains("Business credits"),
            "do not add a third Business credits field"
        );
        let failed_console = field(&failed, "Console API credits:");
        assert_eq!(
            failed_console, "Console API credits: not available",
            "a failed console response is not a balance:\n{failed}"
        );
        assert!(
            !failed_console.contains('$') && failed_console != "Console API credits: $0",
            "a failed console response must not show a dollar amount or $0:\n{failed_console}"
        );
        assert!(
            !failed.contains("Business credits"),
            "a failed console response must not add a Business credits field:\n{failed}"
        );
    }

    #[test]
    fn use_credits_spends_console_api_credits_while_limits_remain_and_does_not_spend_personal() {
        use crossterm::event::{MouseButton, MouseEventKind};
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, apply_meter_source, load_limits_pins,
            save_limits_pins,
        };
        use xai_grok_shell::auth::{
            LimitsSnapshotDocument, LimitsSnapshotManagement, read_limits_snapshot_file,
            write_limits_snapshot_file,
        };
        use xai_grok_shell::sampling::SamplerConfig;

        struct EnvGuard {
            prev_home: Option<std::ffi::OsString>,
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                // Safety: this test runs alone (`--test-threads=1`) and restores GROK_HOME.
                unsafe {
                    match self.prev_home.take() {
                        Some(value) => std::env::set_var("GROK_HOME", value),
                        None => std::env::remove_var("GROK_HOME"),
                    }
                }
            }
        }

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = EnvGuard {
            prev_home: std::env::var_os("GROK_HOME"),
        };
        // Safety: restored by EnvGuard. No stock preferred_method file is written.
        unsafe {
            std::env::set_var("GROK_HOME", home.path());
        }

        let personal_cents = 4321_i64;
        let console_prepaid_cents = 8900_i64;
        let session_key = "included-period-session-token";
        let console_key = "console-api-credits-key";
        let end = DateTime::parse_from_rfc3339("2026-08-08T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut bal = weekly_bal(28.0, end);
        bal.prepaid_balance_cents = Some(personal_cents);
        assert_eq!(bal.usage_pct, 28.0, "included period limits still remain");
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));

        let mut doc = LimitsSnapshotDocument::empty(1);
        doc.management = Some(LimitsSnapshotManagement {
            prepaid_cents: Some(console_prepaid_cents),
            ..Default::default()
        });
        write_limits_snapshot_file(home.path(), &doc).expect("console prepaid snapshot");
        apply_meter_source(MeterSource::Console).expect("meter pin");
        let pins = load_limits_pins();
        assert!(!pins.use_console, "Use credits must not set use_console");
        assert_eq!(pins.meter_source, Some(MeterSource::Console));

        let fresh = || SamplerConfig {
            api_key: Some(session_key.into()),
            failover_api_keys: vec![console_key.into()],
            base_url: "https://cli-chat-proxy.grok.com/v1".into(),
            model: "grok-4".into(),
            session_identity_key: Some(session_key.into()),
            failover_base_url: Some("https://api.x.ai/v1".into()),
            session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
            ..Default::default()
        };
        let mut sampling = fresh();
        apply_limits_pins_to_sampler_config(&mut sampling);
        assert_eq!(
            sampling.api_key.as_deref(),
            Some(console_key),
            "sampler does not read the meter pin"
        );

        std::fs::remove_file(home.path().join("limits_pins.json")).expect("new session");
        assert_eq!(load_limits_pins().meter_source, None);

        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_balance_cents(Some(console_prepaid_cents));
        let mut state = LimitsModalState::new(snap);
        assert_eq!(
            state
                .snapshot
                .primary
                .dollar_credits
                .as_ref()
                .map(|meter| meter.balance_cents),
            Some(personal_cents)
        );
        assert_eq!(
            state
                .snapshot
                .primary
                .included
                .as_ref()
                .map(|meter| meter.used_pct),
            Some(28.0)
        );

        let paint = |state: &mut LimitsModalState| -> String {
            let theme = Theme::default();
            let area = Rect::new(0, 0, 100, 40);
            let mut buf = Buffer::empty(area);
            render_limits_modal(&mut buf, area, state, &theme, false, now);
            (0..area.height)
                .map(|y| {
                    (0..area.width)
                        .map(|x| buf[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let click_other = |state: &mut LimitsModalState| {
            let hit = state
                .window
                .shortcut_hits
                .iter()
                .find(|hit| hit.clickable && hit.id == SPEND_OTHER_CHOICE_ID)
                .map(|hit| hit.rect)
                .expect("the other spend choice is a button");
            let outcome =
                handle_limits_mouse(state, MouseEventKind::Down(MouseButton::Left), hit.x, hit.y);
            assert_eq!(outcome, LimitsModalOutcome::Changed);
        };

        let credits = paint(&mut state);
        assert!(credits.contains("Using limits"), "{credits}");
        assert!(credits.contains("Use credits"), "{credits}");
        assert!(!credits.contains("Using credits"), "{credits}");
        let limits_tab = state.window.tab_rects[1].expect("Limits tab rectangle");
        let outcome = modal_window::handle_modal_mouse(
            &mut state.window,
            MouseEventKind::Down(MouseButton::Left),
            limits_tab.x,
            limits_tab.y,
        );
        assert_eq!(outcome, ModalWindowOutcome::TabChanged(1));
        let limits = paint(&mut state);
        assert!(limits.contains("Using limits"), "{limits}");
        assert!(limits.contains("Use credits"), "{limits}");
        assert!(
            !home.path().join("limits_pins.json").exists(),
            "changing tabs must not write the spend pin"
        );
        let credits_tab = state.window.tab_rects[0].expect("Credits tab rectangle");
        let outcome = modal_window::handle_modal_mouse(
            &mut state.window,
            MouseEventKind::Down(MouseButton::Left),
            credits_tab.x,
            credits_tab.y,
        );
        assert_eq!(outcome, ModalWindowOutcome::TabChanged(0));
        let _credits = paint(&mut state);
        click_other(&mut state);

        let pins = load_limits_pins();
        assert!(!pins.use_console);
        assert_eq!(pins.meter_source, Some(MeterSource::Console));
        assert_ne!(pins.meter_source, Some(MeterSource::DollarCredits));
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));
        assert_eq!(
            read_limits_snapshot_file(home.path())
                .and_then(|doc| doc.management)
                .and_then(|mgmt| mgmt.prepaid_cents),
            Some(console_prepaid_cents)
        );
        let mut next = fresh();
        apply_limits_pins_to_sampler_config(&mut next);
        assert_eq!(
            next.api_key.as_deref(),
            Some(console_key),
            "Use credits makes the next request use console API credits while included period limits remain"
        );
        assert!(next.base_url.contains("api.x.ai"), "{}", next.base_url);
        assert_ne!(next.api_key.as_deref(), Some(session_key));

        let using_credits = paint(&mut state);
        assert!(using_credits.contains("Using credits"), "{using_credits}");
        assert!(using_credits.contains("Use limits"), "{using_credits}");
        assert!(!using_credits.contains("Using limits"), "{using_credits}");
        assert!(!using_credits.contains("Use credits"), "{using_credits}");
        click_other(&mut state);
        let pins = load_limits_pins();
        assert!(!pins.use_console);
        assert_eq!(pins.meter_source, Some(MeterSource::Included));
        assert_ne!(pins.meter_source, Some(MeterSource::DollarCredits));
        apply_limits_pins_to_sampler_config(&mut next);
        assert_eq!(
            next.api_key.as_deref(),
            Some(session_key),
            "Use limits makes the next request use included period limits"
        );
        assert!(
            next.base_url.contains("cli-chat-proxy"),
            "{}",
            next.base_url
        );

        let mut failed_doc = LimitsSnapshotDocument::empty(1);
        failed_doc.management = Some(LimitsSnapshotManagement {
            prepaid_cents: None,
            ..Default::default()
        });
        write_limits_snapshot_file(home.path(), &failed_doc).expect("failed console snapshot");
        let mut failed_bal = weekly_bal(28.0, end);
        failed_bal.prepaid_balance_cents = Some(personal_cents);
        let failed_snap = LimitsSnapshot::from_billing(
            Some(&failed_bal),
            None,
            SamplingIdentityKind::SuperGrokSession,
        )
        .with_console_balance_cents(None)
        .with_console_prepaid_gap(ConsoleTeamPrepaidGap::Unavailable);
        state.apply_snapshot(failed_snap);
        let mut stale = load_limits_pins();
        stale.meter_source = Some(MeterSource::Console);
        stale.use_console = false;
        save_limits_pins(&stale).expect("stale console pin");
        let mut failed_req = fresh();
        apply_limits_pins_to_sampler_config(&mut failed_req);
        assert_eq!(
            failed_req.api_key.as_deref(),
            Some(session_key),
            "a failed console response does not switch the spend onto personal credits"
        );
        assert!(
            failed_req.base_url.contains("cli-chat-proxy"),
            "{}",
            failed_req.base_url
        );
        assert!(!load_limits_pins().use_console);
        assert_ne!(
            load_limits_pins().meter_source,
            Some(MeterSource::DollarCredits)
        );

        std::fs::remove_file(home.path().join("limits_pins.json")).expect("new session");
        let failed_paint = paint(&mut state);
        assert!(failed_paint.contains("Using limits"), "{failed_paint}");
        assert!(failed_paint.contains("Use credits"), "{failed_paint}");
        assert!(
            failed_paint.contains("Console API credits: not available"),
            "{failed_paint}"
        );
        click_other(&mut state);
        assert!(!load_limits_pins().use_console);
        assert_ne!(
            load_limits_pins().meter_source,
            Some(MeterSource::DollarCredits)
        );
        assert_eq!(
            state
                .snapshot
                .primary
                .dollar_credits
                .as_ref()
                .map(|meter| meter.balance_cents),
            Some(personal_cents)
        );
        assert_eq!(failed_bal.prepaid_balance_cents, Some(personal_cents));
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));
        let mut after_failed_click = fresh();
        apply_limits_pins_to_sampler_config(&mut after_failed_click);
        assert_eq!(after_failed_click.api_key.as_deref(), Some(session_key));
        assert_ne!(after_failed_click.api_key.as_deref(), Some(console_key));
    }

    /// Use limits stays on. A real SuperGrok HTTP 402 memo (the durable record
    /// `mark_exhausted` writes) plus available console API credits selects the
    /// console key for the next request. A client 100% printout does not mark
    /// a pool used up and does not change that request. Personal cents stay
    /// put. A failed console fetch does not spend them either.
    #[test]
    fn real_402_uses_console_api_credits_when_available_and_a_100_percent_printout_does_not() {
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, apply_meter_source, load_limits_pins,
        };
        use xai_grok_shell::auth::{
            AllowanceExhaustAction, LimitsSnapshotDocument, LimitsSnapshotManagement,
            apply_billing_usage_to_session_exhaust, read_limits_snapshot_file,
            write_limits_snapshot_file,
        };
        use xai_grok_shell::sampling::SamplerConfig;

        struct EnvGuard {
            prev_home: Option<std::ffi::OsString>,
            prev_force: Option<std::ffi::OsString>,
            prev_xai: Option<std::ffi::OsString>,
            prev_legacy: Option<std::ffi::OsString>,
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                // Safety: this test runs alone (`--test-threads=1`) and restores env.
                restore("GROK_HOME", self.prev_home.take());
                restore("GROK_CREDENTIALS_FORCE_FILE", self.prev_force.take());
                restore("XAI_API_KEY", self.prev_xai.take());
                restore("GROK_CODE_XAI_API_KEY", self.prev_legacy.take());
            }
        }
        fn restore(key: &str, prev: Option<std::ffi::OsString>) {
            // Safety: tests run with `--test-threads=1` and restore the prior env.
            unsafe {
                match prev {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
        // Same FNV-1a 64 as `grok_rate_limit::fingerprint_secret`. That name is
        // the real HTTP 402 file under `$GROK_HOME/exhausted_credits/`.
        fn real_http_402_memo_fingerprint(secret: &str) -> String {
            let mut hash: u64 = 0xcbf29ce484222325;
            for byte in secret.as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            format!("{hash:016x}")
        }
        fn plant_real_supergrok_http_402(home: &std::path::Path, session_key: &str) {
            let dir = home.join("exhausted_credits");
            std::fs::create_dir_all(&dir).expect("402 memo dir");
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix clock")
                .as_millis();
            let until = u64::try_from(now_ms)
                .expect("ms fits u64")
                .saturating_add(3_600_000);
            let name = real_http_402_memo_fingerprint(session_key);
            std::fs::write(
                dir.join(format!("{name}.json")),
                format!("{{\n  \"until_unix_ms\": {until}\n}}\n"),
            )
            .expect("real SuperGrok HTTP 402 memo");
        }
        fn printout_marked_a_pool(home: &std::path::Path) -> bool {
            let dir = home.join("exhausted_credits");
            let Ok(entries) = std::fs::read_dir(&dir) else {
                return false;
            };
            entries
                .flatten()
                .any(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
        }

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = EnvGuard {
            prev_home: std::env::var_os("GROK_HOME"),
            prev_force: std::env::var_os("GROK_CREDENTIALS_FORCE_FILE"),
            prev_xai: std::env::var_os("XAI_API_KEY"),
            prev_legacy: std::env::var_os("GROK_CODE_XAI_API_KEY"),
        };
        // Safety: restored by EnvGuard. No stock preferred_method file is written.
        unsafe {
            std::env::set_var("GROK_HOME", home.path());
            std::env::set_var("GROK_CREDENTIALS_FORCE_FILE", "1");
            std::env::remove_var("XAI_API_KEY");
            std::env::remove_var("GROK_CODE_XAI_API_KEY");
        }

        let personal_cents = 4321_i64;
        let console_prepaid_cents = 8900_i64;
        let session_key = "real-402-included-period-session-token";
        let console_key = "real-402-console-api-credits-key";
        let end = DateTime::parse_from_rfc3339("2026-08-08T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut bal = weekly_bal(100.0, end);
        bal.prepaid_balance_cents = Some(personal_cents);
        assert_eq!(bal.usage_pct, 100.0, "client printout is 100%");
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));

        std::fs::write(
            home.path().join("auth.json"),
            format!(
                r#"{{
  "https://auth.x.ai::real-402": {{
    "key": "{session_key}",
    "auth_mode": "oidc",
    "create_time": "2026-08-01T00:00:00Z",
    "user_id": "real-402-user"
  }}
}}
"#
            ),
        )
        .expect("session auth.json");

        let mut doc = LimitsSnapshotDocument::empty(1);
        doc.management = Some(LimitsSnapshotManagement {
            prepaid_cents: Some(console_prepaid_cents),
            ..Default::default()
        });
        write_limits_snapshot_file(home.path(), &doc).expect("console prepaid snapshot");
        apply_meter_source(MeterSource::Included).expect("Use limits pin");
        let pins = load_limits_pins();
        assert!(!pins.use_console, "Use limits must not set use_console");
        assert_eq!(pins.meter_source, Some(MeterSource::Included));
        assert_ne!(pins.meter_source, Some(MeterSource::DollarCredits));

        let fresh = || SamplerConfig {
            api_key: Some(session_key.into()),
            failover_api_keys: vec![console_key.into()],
            base_url: "https://cli-chat-proxy.grok.com/v1".into(),
            model: "grok-4".into(),
            session_identity_key: Some(session_key.into()),
            failover_base_url: Some("https://api.x.ai/v1".into()),
            session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
            ..Default::default()
        };

        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_balance_cents(Some(console_prepaid_cents));
        let mut state = LimitsModalState::new(snap);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        render_limits_modal(&mut buf, area, &mut state, &theme, false, now);
        let painted = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(painted.contains("Using limits"), "{painted}");
        assert!(painted.contains("Use credits"), "{painted}");
        assert!(!painted.contains("Using credits"), "{painted}");

        let marked = apply_billing_usage_to_session_exhaust(100.0, home.path());
        assert_eq!(
            marked,
            AllowanceExhaustAction::None,
            "a client printout of 100% must not mark a pool used up"
        );
        assert!(
            !printout_marked_a_pool(home.path()),
            "a client printout of 100% must not write an exhausted-pool memo"
        );
        let mut printout_req = fresh();
        apply_limits_pins_to_sampler_config(&mut printout_req);
        assert_eq!(
            printout_req.api_key.as_deref(),
            Some(session_key),
            "a client printout of 100% must not change the next request"
        );
        assert!(
            printout_req.base_url.contains("cli-chat-proxy"),
            "{}",
            printout_req.base_url
        );
        assert_ne!(printout_req.api_key.as_deref(), Some(console_key));
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));
        assert_eq!(load_limits_pins().meter_source, Some(MeterSource::Included));
        assert!(!load_limits_pins().use_console);
        assert_ne!(
            load_limits_pins().meter_source,
            Some(MeterSource::DollarCredits)
        );

        plant_real_supergrok_http_402(home.path(), session_key);
        let mut next = fresh();
        apply_limits_pins_to_sampler_config(&mut next);
        assert_eq!(
            next.api_key.as_deref(),
            Some(console_key),
            "a real SuperGrok HTTP 402 must put the next request on the console API credits key when those credits are available"
        );
        assert!(next.base_url.contains("api.x.ai"), "{}", next.base_url);
        assert_ne!(next.api_key.as_deref(), Some(session_key));
        assert!(!load_limits_pins().use_console);
        assert_eq!(load_limits_pins().meter_source, Some(MeterSource::Included));
        assert_ne!(
            load_limits_pins().meter_source,
            Some(MeterSource::DollarCredits)
        );
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));
        assert_eq!(
            read_limits_snapshot_file(home.path())
                .and_then(|doc| doc.management)
                .and_then(|mgmt| mgmt.prepaid_cents),
            Some(console_prepaid_cents)
        );
        assert_eq!(
            state
                .snapshot
                .primary
                .dollar_credits
                .as_ref()
                .map(|meter| meter.balance_cents),
            Some(personal_cents)
        );

        let mut failed_doc = LimitsSnapshotDocument::empty(1);
        failed_doc.management = Some(LimitsSnapshotManagement {
            prepaid_cents: None,
            ..Default::default()
        });
        write_limits_snapshot_file(home.path(), &failed_doc).expect("failed console snapshot");
        let mut failed_bal = weekly_bal(100.0, end);
        failed_bal.prepaid_balance_cents = Some(personal_cents);
        let failed_snap = LimitsSnapshot::from_billing(
            Some(&failed_bal),
            None,
            SamplingIdentityKind::SuperGrokSession,
        )
        .with_console_balance_cents(None)
        .with_console_prepaid_gap(ConsoleTeamPrepaidGap::Unavailable);
        state.apply_snapshot(failed_snap);
        let mut failed_req = fresh();
        apply_limits_pins_to_sampler_config(&mut failed_req);
        assert_eq!(
            failed_req.api_key.as_deref(),
            Some(session_key),
            "a failed console fetch must not spend personal credits"
        );
        assert_ne!(failed_req.api_key.as_deref(), Some(console_key));
        assert!(
            failed_req.base_url.contains("cli-chat-proxy"),
            "{}",
            failed_req.base_url
        );
        assert!(!load_limits_pins().use_console);
        assert_ne!(
            load_limits_pins().meter_source,
            Some(MeterSource::DollarCredits)
        );
        assert_eq!(failed_bal.prepaid_balance_cents, Some(personal_cents));
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));
        assert_eq!(
            state
                .snapshot
                .primary
                .dollar_credits
                .as_ref()
                .map(|meter| meter.balance_cents),
            Some(personal_cents)
        );
    }

    /// Included SuperGrok period limits and console API credits are both out
    /// after a real refusal, not after a 100% printout. A known period end
    /// 2 days, 4 hours, and 12 minutes out makes the chip show `2d 4h 12m`.
    /// No further request is sent. Personal cents stay 4321. A missing period
    /// end says the reset time is not available, with no invented clock.
    #[test]
    fn both_limits_and_console_api_credits_exhausted_shows_days_hours_minutes_until_reset() {
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, apply_meter_source, load_limits_pins,
        };
        use xai_grok_shell::auth::{
            AllowanceExhaustAction, LimitsSnapshotDocument, LimitsSnapshotManagement,
            apply_billing_usage_to_session_exhaust, read_limits_snapshot_file,
            write_limits_snapshot_file,
        };
        use xai_grok_shell::sampling::SamplerConfig;

        use crate::actions::ActionRegistry;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::app::bundle::BundleState;
        use crate::scrollback::render::ScratchBuffer;

        struct EnvGuard {
            prev_home: Option<std::ffi::OsString>,
            prev_force: Option<std::ffi::OsString>,
            prev_xai: Option<std::ffi::OsString>,
            prev_legacy: Option<std::ffi::OsString>,
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                // Safety: this test runs alone (`--test-threads=1`) and restores env.
                restore("GROK_HOME", self.prev_home.take());
                restore("GROK_CREDENTIALS_FORCE_FILE", self.prev_force.take());
                restore("XAI_API_KEY", self.prev_xai.take());
                restore("GROK_CODE_XAI_API_KEY", self.prev_legacy.take());
            }
        }
        fn restore(key: &str, prev: Option<std::ffi::OsString>) {
            // Safety: tests run with `--test-threads=1` and restore the prior env.
            unsafe {
                match prev {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
        // Same FNV-1a 64 as `grok_rate_limit::fingerprint_secret`.
        fn real_http_402_memo_fingerprint(secret: &str) -> String {
            let mut hash: u64 = 0xcbf29ce484222325;
            for byte in secret.as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            format!("{hash:016x}")
        }
        fn plant_real_http_402(home: &std::path::Path, secret: &str) {
            let dir = home.join("exhausted_credits");
            std::fs::create_dir_all(&dir).expect("402 memo dir");
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix clock")
                .as_millis();
            let until = u64::try_from(now_ms)
                .expect("ms fits u64")
                .saturating_add(3_600_000);
            let name = real_http_402_memo_fingerprint(secret);
            std::fs::write(
                dir.join(format!("{name}.json")),
                format!("{{\n  \"until_unix_ms\": {until}\n}}\n"),
            )
            .expect("real HTTP 402 memo");
        }
        fn memo_dir_has_json(home: &std::path::Path) -> bool {
            let dir = home.join("exhausted_credits");
            let Ok(entries) = std::fs::read_dir(&dir) else {
                return false;
            };
            entries
                .flatten()
                .any(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
        }
        fn paint_status(balance: &CreditBalance) -> String {
            crate::appearance::cache::set_hide_header(false);
            let mut agent = make_agent();
            agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
            agent.credit_balance = Some(balance.clone());
            let area = Rect::new(0, 0, 160, 40);
            let mut header = Buffer::empty(area);
            let mut scratch = ScratchBuffer::new();
            agent.draw(
                area,
                &mut header,
                &ActionRegistry::defaults(),
                &mut scratch,
                None,
                false,
                BannerSlotParams::none(),
                &BundleState::default(),
                false,
                false,
                &mut Vec::new(),
                AppRenderParams::default(),
            );
            (0..area.height)
                .map(|y| {
                    (0..area.width)
                        .filter_map(|x| header.cell((x, y)).map(|cell| cell.symbol().to_string()))
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = EnvGuard {
            prev_home: std::env::var_os("GROK_HOME"),
            prev_force: std::env::var_os("GROK_CREDENTIALS_FORCE_FILE"),
            prev_xai: std::env::var_os("XAI_API_KEY"),
            prev_legacy: std::env::var_os("GROK_CODE_XAI_API_KEY"),
        };
        // Safety: restored by EnvGuard. No stock preferred_method file is written.
        unsafe {
            std::env::set_var("GROK_HOME", home.path());
            std::env::set_var("GROK_CREDENTIALS_FORCE_FILE", "1");
            std::env::remove_var("XAI_API_KEY");
            std::env::remove_var("GROK_CODE_XAI_API_KEY");
        }

        let personal_cents = 4321_i64;
        let console_prepaid_cents = 8900_i64;
        let session_key = "both-out-included-period-session-token";
        let console_key = "both-out-console-api-credits-key";
        std::fs::write(
            home.path().join("auth.json"),
            format!(
                r#"{{
  "https://auth.x.ai::both-out": {{
    "key": "{session_key}",
    "auth_mode": "oidc",
    "create_time": "2026-08-01T00:00:00Z",
    "user_id": "both-out-user"
  }},
  "xai::api_key": {{
    "key": "{console_key}",
    "auth_mode": "api_key",
    "create_time": "2026-08-01T00:00:00Z",
    "user_id": "both-out-user"
  }}
}}
"#
            ),
        )
        .expect("session and console auth.json");

        let mut doc = LimitsSnapshotDocument::empty(1);
        doc.management = Some(LimitsSnapshotManagement {
            prepaid_cents: Some(console_prepaid_cents),
            ..Default::default()
        });
        write_limits_snapshot_file(home.path(), &doc).expect("console prepaid snapshot");
        apply_meter_source(MeterSource::Included).expect("Use limits pin");
        assert!(!load_limits_pins().use_console);
        assert_eq!(load_limits_pins().meter_source, Some(MeterSource::Included));
        assert_ne!(
            load_limits_pins().meter_source,
            Some(MeterSource::DollarCredits)
        );

        let fresh = || SamplerConfig {
            api_key: Some(session_key.into()),
            failover_api_keys: vec![console_key.into()],
            base_url: "https://cli-chat-proxy.grok.com/v1".into(),
            model: "grok-4".into(),
            session_identity_key: Some(session_key.into()),
            failover_base_url: Some("https://api.x.ai/v1".into()),
            session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
            ..Default::default()
        };

        let period_end = Utc::now()
            + chrono::Duration::days(2)
            + chrono::Duration::hours(4)
            + chrono::Duration::minutes(12)
            + chrono::Duration::seconds(30);
        let mut bal = weekly_bal(100.0, period_end);
        bal.prepaid_balance_cents = Some(personal_cents);
        assert_eq!(bal.usage_pct, 100.0, "client printout is 100%");
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));

        let marked = apply_billing_usage_to_session_exhaust(100.0, home.path());
        assert_eq!(
            marked,
            AllowanceExhaustAction::None,
            "a client printout of 100% must not mark a pool out"
        );
        assert!(
            !memo_dir_has_json(home.path()),
            "a client printout of 100% must not write a refusal memo"
        );
        let mut printout_req = fresh();
        apply_limits_pins_to_sampler_config(&mut printout_req);
        assert_eq!(
            printout_req.api_key.as_deref(),
            Some(session_key),
            "a client printout of 100% must not change the next request"
        );
        assert!(printout_req.base_url.contains("cli-chat-proxy"));
        assert_ne!(printout_req.api_key.as_deref(), Some(console_key));
        let printout_chip = paint_status(&bal);
        assert!(
            printout_chip.contains("limits 100%"),
            "a 100% printout keeps the used-percent chip:\n{printout_chip}"
        );
        assert!(
            !printout_chip.contains("2d 4h 12m"),
            "a 100% printout must not show the reset countdown:\n{printout_chip}"
        );

        plant_real_http_402(home.path(), session_key);
        plant_real_http_402(home.path(), console_key);
        let both_out = paint_status(&bal);
        assert!(
            both_out.contains("2d 4h 12m"),
            "a known period end 2 days, 4 hours, and 12 minutes out must make the chip show `2d 4h 12m`:\n{both_out}"
        );
        assert!(
            !both_out.contains("limits 100%"),
            "both real refusals replace the used-percent chip:\n{both_out}"
        );

        let mut next = fresh();
        apply_limits_pins_to_sampler_config(&mut next);
        assert!(
            next.api_key.is_none(),
            "no further request is sent, api_key={:?}",
            next.api_key
        );
        assert_ne!(next.api_key.as_deref(), Some(session_key));
        assert_ne!(next.api_key.as_deref(), Some(console_key));
        assert!(
            next.base_url.contains("cli-chat-proxy"),
            "no further request must stay off the console host: {}",
            next.base_url
        );
        assert!(!load_limits_pins().use_console);
        assert_eq!(load_limits_pins().meter_source, Some(MeterSource::Included));
        assert_ne!(
            load_limits_pins().meter_source,
            Some(MeterSource::DollarCredits)
        );
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));
        assert_eq!(
            read_limits_snapshot_file(home.path())
                .and_then(|saved| saved.management)
                .and_then(|mgmt| mgmt.prepaid_cents),
            Some(console_prepaid_cents)
        );
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_balance_cents(Some(console_prepaid_cents));
        let state = LimitsModalState::new(snap);
        assert_eq!(
            state
                .snapshot
                .primary
                .dollar_credits
                .as_ref()
                .map(|meter| meter.balance_cents),
            Some(personal_cents)
        );

        let mut missing_end = bal.clone();
        missing_end.period_end_at = None;
        missing_end.period_end_display = None;
        let missing = paint_status(&missing_end);
        assert!(
            missing.contains("the reset time is not available"),
            "a missing period end must say the reset time is not available:\n{missing}"
        );
        assert!(
            !missing.contains("used up"),
            "a missing period end must not say used up:\n{missing}"
        );
        let invented_clock = missing
            .lines()
            .any(|line| line.contains("d ") && line.contains("h ") && line.contains('m'));
        assert!(
            !invented_clock,
            "a missing period end must not invent a clock:\n{missing}"
        );
    }
}
