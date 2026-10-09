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
    /// Painted `Use limits` control from the last render. Absent when that
    /// button is not on screen.
    pub use_limits_hit: Option<Rect>,
}

impl LimitsModalState {
    pub fn new(snapshot: LimitsSnapshot) -> Self {
        Self {
            window: ModalWindowState::new(),
            snapshot,
            scroll: 0,
            zero_refresh_sent: false,
            last_updated_at: Utc::now(),
            use_limits_hit: None,
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

/// Limits is the included allowance, the bar, and the short week line.
/// Credits is personal credits and console API credits. Limits is first
/// and is the default open tab.
const CARD_TABS: &[&str] = &["Limits", "Credits"];
/// Default open tab. Limits mode shows this tab.
pub(crate) const LIMITS_TAB: usize = 0;
/// Second tab. SuperGrok dollar credits and console API credits stay here.
const CREDITS_TAB: usize = 1;

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

/// `$GROK_HOME` on each call. The process grok home OnceLock misses a
/// fixture home written after the lock.
fn limits_reading_grok_home() -> std::path::PathBuf {
    match std::env::var("GROK_HOME") {
        Ok(home) => std::path::PathBuf::from(home),
        Err(_) => xai_grok_shell::util::grok_home::grok_home(),
    }
}

/// Team id on a live Team JWT. Hard-expired rows and the personal slot
/// are not this id.
fn live_team_jwt_team_id() -> Option<String> {
    let map =
        xai_grok_shell::auth::read_auth_json(&limits_reading_grok_home().join("auth.json")).ok()?;
    for (scope, auth) in &map {
        if scope.contains("::personal") {
            continue;
        }
        if !xai_grok_shell::auth::is_supergrok_session_mode(auth.auth_mode) {
            continue;
        }
        if auth.key.trim().is_empty() || !auth.is_team_principal() {
            continue;
        }
        if auth.expires_at.is_some_and(|end| chrono::Utc::now() >= end) {
            continue;
        }
        if let Some(id) = auth
            .team_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            return Some(id.to_owned());
        }
    }
    None
}

/// Whole dollars as `$N`, otherwise `$N.NN`. A missing reading is not `$0`.
pub(crate) fn format_remaining_cents_as_dollars(cents: i64) -> String {
    let negative = cents < 0;
    let abs_cents = cents.unsigned_abs();
    let dollars = abs_cents / 100;
    let frac = abs_cents % 100;
    let body = if frac == 0 {
        format!("${dollars}")
    } else {
        format!("${dollars}.{frac:02}")
    };
    if negative { format!("-{body}") } else { body }
}

/// On-disk team postpaid Billing Credits remaining.
///
/// The open Limits card and the header share this after a process-cache
/// miss. `limits_snapshot.json` counts only when the Billing Credits card
/// was fetched and names a team. Prepaid cents are console team prepaid,
/// not this remaining. An unread card stays unread. This does not invent `$0`.
fn team_postpaid_billing_credits_cents_from_snapshot() -> Option<i64> {
    let doc = xai_grok_shell::auth::read_limits_snapshot_file(limits_reading_grok_home())?;
    let mgmt = doc.management?;
    if mgmt.billing_credits_card.as_wire() != "fetched" {
        return None;
    }
    let team = mgmt.team_id.as_deref().map(str::trim).unwrap_or("");
    if team.is_empty() {
        return None;
    }
    mgmt.billing_credits_cents
}

/// Team postpaid Billing Credits remaining shared by the header chip and
/// the Limits card. The process cache is one slot, keyed by the Management
/// API team id, which is not the Team JWT team id. Try that management id
/// first, then the JWT id, then the fetched snapshot the open card already
/// shows. `None` stays unread. This does not invent `$0`. Not included
/// SuperGrok period limits, not SuperGrok dollar credits, and not console
/// team prepaid.
pub(crate) fn team_postpaid_billing_credits_remaining_dollars() -> Option<String> {
    let cents = xai_grok_shell::auth::cached_console_team_postpaid_default()
        .and_then(|meter| meter.billing_credits_remaining_cents)
        .or_else(|| {
            let team_id = live_team_jwt_team_id()?;
            xai_grok_shell::auth::cached_console_team_postpaid(&team_id)?
                .billing_credits_remaining_cents
        })
        .or_else(team_postpaid_billing_credits_cents_from_snapshot)?;
    Some(format_remaining_cents_as_dollars(cents))
}

fn limits_tab_lines(
    state: &LimitsModalState,
    now: DateTime<Utc>,
    draws_included: bool,
) -> Vec<String> {
    let mut lines = match (draws_included, state.snapshot.primary.included.as_ref()) {
        (true, Some(included)) => {
            let used = included.used_pct_floored();
            let rem = included.remaining_pct_floored();
            let allowance = match included.period_label {
                "Included" => format!("  Included allowance: {used}% used · {rem}% remaining"),
                other => format!(
                    "  Included {} allowance: {used}% used · {rem}% remaining",
                    other.to_lowercase()
                ),
            };
            let reset = match &included.next_reset_display {
                Some(text) => format!("  Next reset: {text}"),
                None => "  Next reset: not known yet".to_string(),
            };
            vec![allowance, reset, short_week_line(included, now)]
        }
        // A known reading is not the meter for the next request. Do not paint
        // a used percent, a remaining percent, or a week percent from it.
        (false, Some(included)) => match &included.next_reset_display {
            Some(text) => vec![format!("  Next reset: {text}")],
            None => vec!["  Next reset: not known yet".to_string()],
        },
        // Unread usage stays off the used-percent line. The week line stays.
        (_, None) => vec![
            "  Included weekly allowance: not available yet".to_string(),
            "Pacing for this week is not known yet.".to_string(),
        ],
    };
    // Failed console fetch: same field the Credits tab uses, on this body too.
    let console = credits_tab_meter_line(&console_credits_line(state));
    if console.trim() == "Console API credits: not available" {
        lines.push(console);
    }
    // Same remaining the header chip paints. Omit the line when unread.
    if let Some(dollars) = team_postpaid_billing_credits_remaining_dollars() {
        lines.push(format!("  Team postpaid Billing Credits: {dollars} left"));
    }
    lines
}

fn short_week_line(
    included: &crate::views::limits_snapshot::IncludedAllowanceMeter,
    now: DateTime<Utc>,
) -> String {
    if included.period_label != "Weekly" {
        return "Pacing for this week is not known yet.".to_string();
    }
    let Some(end) = included.next_reset_at else {
        return "Pacing for this week is not known yet.".to_string();
    };
    let Some(start) = xai_grok_shell::token_economy::resolve_period_start(
        None,
        Some(end),
        Some("USAGE_PERIOD_TYPE_WEEKLY"),
    ) else {
        return "Pacing for this week is not known yet.".to_string();
    };
    let Some(pacing) =
        xai_grok_shell::token_economy::compute_period_pacing(included.used_pct, start, end, now)
    else {
        return "Pacing for this week is not known yet.".to_string();
    };
    linear_week_label(pacing)
}

fn fmt_card_cents(cents: i64) -> String {
    let dollars = cents.abs() as f64 / 100.0;
    if dollars.fract() == 0.0 {
        format!("${dollars:.0}")
    } else {
        format!("${dollars:.2}")
    }
}

/// Credits tab body. Personal credits and console API credits only.
fn credits_tab_lines(state: &LimitsModalState) -> Vec<String> {
    let personal = match &state.snapshot.primary.dollar_credits {
        Some(meter) => format!(
            "  SuperGrok dollar credits: {}",
            fmt_card_cents(meter.balance_cents)
        ),
        None if state.snapshot.primary.dollar_credits_observed => {
            "  SuperGrok dollar credits: none on file".to_string()
        }
        None => "  SuperGrok dollar credits: no data yet".to_string(),
    };
    let console = console_credits_line(state);
    vec![personal, console]
}

/// Team prepaid line for the Credits tab.
///
/// `from_billing` leaves the gap as missing management key and does not copy
/// cents. When a console inference key already fetched the team balance, that
/// cache is the amount. An explicit key-available gap with no cents stays
/// `not available`.
fn console_credits_line(state: &LimitsModalState) -> String {
    if let Some(cents) = state
        .snapshot
        .console
        .balance_cents
        .or_else(|| inference_key_balance_for_unmarked_card(state))
    {
        return format!("  Team prepaid remaining: {}", fmt_card_cents(cents));
    }
    let inference_key = state.snapshot.console.key_available || state.snapshot.console.is_live;
    let gap = state
        .snapshot
        .console
        .prepaid_gap
        .credits_tab_unknown_phrase(inference_key);
    format!("  Team prepaid remaining: {gap}")
}

/// Cached inference-key team balance for a card that has not marked the key.
fn inference_key_balance_for_unmarked_card(state: &LimitsModalState) -> Option<i64> {
    let console = &state.snapshot.console;
    if console.balance_cents.is_some() || console.key_available || console.is_live {
        return None;
    }
    if console.prepaid_gap != crate::views::credit_bar::ConsoleTeamPrepaidGap::MissingManagementKey
    {
        return None;
    }
    crate::views::credit_bar::team_prepaid_cents_from_inference_key_cache()
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

/// True when the card may say `Using limits`.
///
/// That is the next request drawing included SuperGrok period limits. An
/// accepted Included choice is that request, including beside a Team JWT.
/// An unmarked card that shows a team inference balance is spending that
/// team credit balance. A missing pin must not call that `Using limits`.
/// The explicit Included pin still does, so that balance does not put
/// `Use limits` back.
fn showing_using_limits(state: &LimitsModalState) -> bool {
    let pins = xai_grok_shell::auth::limits_pins::load_limits_pins();
    let explicit_included =
        pins.meter_source == Some(xai_grok_shell::auth::limits_pins::MeterSource::Included);
    let team_balance =
        inference_key_balance_for_unmarked_card(state).is_some_and(|cents| cents > 0);
    if team_balance && !explicit_included {
        return false;
    }
    xai_grok_shell::auth::limits_pins::next_request_draws_included_period_limits()
}

/// The operator asked for included SuperGrok period limits, and the next
/// request still does not draw that meter. `use_console` is that case.
/// An Included pin beside a Team JWT is not that case.
fn included_limits_were_requested_but_are_not_drawn() -> bool {
    let pins = xai_grok_shell::auth::limits_pins::load_limits_pins();
    pins.meter_source == Some(xai_grok_shell::auth::limits_pins::MeterSource::Included)
        && !xai_grok_shell::auth::limits_pins::next_request_draws_included_period_limits()
}

/// Limits label on the left, Credits label on the right.
///
/// The labels are `'static`. The signature must say so. Eliding the
/// lifetime would keep `state` borrowed for the whole array, and the
/// modal cannot mutably borrow `state.window` while that borrow lives.
fn spend_shortcuts(state: &LimitsModalState) -> Vec<Shortcut<'static>> {
    let esc = Shortcut {
        label: "Esc close",
        clickable: true,
        id: 1,
    };
    if showing_using_limits(state) {
        vec![
            Shortcut {
                label: "Using limits",
                clickable: false,
                id: 0,
            },
            Shortcut {
                label: "Use credits",
                clickable: true,
                id: SPEND_OTHER_CHOICE_ID,
            },
            esc,
        ]
    } else if included_limits_were_requested_but_are_not_drawn() {
        // `use_console` still blocks the Included pin. `Using limits` would
        // claim a meter this request does not draw.
        vec![
            Shortcut {
                label: "Using credits",
                clickable: false,
                id: 0,
            },
            esc,
        ]
    } else {
        vec![
            Shortcut {
                label: "Use limits",
                clickable: true,
                id: SPEND_OTHER_CHOICE_ID,
            },
            Shortcut {
                label: "Using credits",
                clickable: false,
                id: 0,
            },
            esc,
        ]
    }
}

fn persist_other_spend_choice(state: &LimitsModalState) -> std::io::Result<()> {
    use xai_grok_shell::auth::limits_pins::{MeterSource, load_limits_pins, save_limits_pins};
    let mut pins = load_limits_pins();
    pins.use_console = false;
    if showing_using_limits(state) {
        if console_api_credits_balance_available(state) {
            pins.meter_source = Some(MeterSource::Console);
            pins.stay_supergrok = false;
        } else if pins.meter_source == Some(MeterSource::Included) {
            // No console balance. Clear Included. Do not write Console.
            // A team-only login does not hop to the console API key.
            pins.meter_source = None;
            pins.stay_supergrok = false;
        } else if pins.meter_source == Some(MeterSource::DollarCredits) {
            pins.meter_source = Some(MeterSource::Included);
        }
    } else {
        pins.meter_source = Some(MeterSource::Included);
        pins.stay_supergrok = false;
    }
    save_limits_pins(&pins)
}

/// Footer control that paints the words `Use limits`.
///
/// The shortcut renderer splits the label at the space (`Use` bold, ` limits`
/// muted). Those cells are still one rect. A transcript line that happens to
/// contain the same words is not this control.
fn use_limits_control_rect(
    buf: &Buffer,
    shortcuts: &[Shortcut<'_>],
    hits: &[crate::views::modal_window::ShortcutHitArea],
) -> Option<Rect> {
    const PHRASE: &str = "Use limits";
    let idx = shortcuts
        .iter()
        .position(|shortcut| shortcut.label == PHRASE)?;
    let hit = hits.iter().find(|hit| hit.shortcuts_idx == idx)?;
    let painted = (0..hit.rect.width)
        .map(|dx| buf[(hit.rect.x.saturating_add(dx), hit.rect.y)].symbol())
        .collect::<String>();
    if painted == PHRASE {
        return Some(hit.rect);
    }
    painted_use_limits_rect(buf, hit.rect).or(Some(hit.rect))
}

/// Cells that paint the words `Use limits`.
fn painted_use_limits_rect(buf: &Buffer, area: Rect) -> Option<Rect> {
    const PHRASE: &str = "Use limits";
    let width = PHRASE.chars().count() as u16;
    if width == 0 || area.width < width || area.height == 0 {
        return None;
    }
    for y in area.y..area.y.saturating_add(area.height) {
        let mut row = String::new();
        for x in area.x..area.x.saturating_add(area.width) {
            row.push_str(buf[(x, y)].symbol());
        }
        if let Some(byte) = row.find(PHRASE) {
            let x_off = row[..byte].chars().count() as u16;
            return Some(Rect {
                x: area.x.saturating_add(x_off),
                y,
                width,
                height: 1,
            });
        }
    }
    None
}

/// Click on the limits card. The spend button writes the meter pin.
/// A left click on the painted `Use limits` control is that button.
pub fn handle_limits_mouse(
    state: &mut LimitsModalState,
    kind: crossterm::event::MouseEventKind,
    column: u16,
    row: u16,
) -> LimitsModalOutcome {
    let left_down = matches!(
        kind,
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
    );
    let on_use_limits = left_down
        && state
            .use_limits_hit
            .is_some_and(|rect| rect.contains((column, row).into()));
    if on_use_limits {
        return if persist_other_spend_choice(state).is_ok() {
            LimitsModalOutcome::Changed
        } else {
            LimitsModalOutcome::Unchanged
        };
    }
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
    let shortcuts = spend_shortcuts(state);
    // 0.70 keeps the 57-column not-drawn sentence on one line in a
    // 100-column terminal. At 0.55 that sentence wraps across the border,
    // so the card no longer contains it.
    let sizing = ModalSizing {
        width_pct: 0.70,
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
        state.use_limits_hit = None;
        return;
    };
    // Record the footer letters, not only the shortcut id. `Use credits`
    // shares that id. `Using limits` is a status, not this control. A click
    // that already asked for included limits, when this request cannot draw
    // them, takes the control off the card.
    state.use_limits_hit = use_limits_control_rect(buf, &shortcuts, &state.window.shortcut_hits);
    let content = mca.content;
    if content.width == 0 || content.height == 0 {
        return;
    }

    // Word-wrap plain content to content width so long notes do not mid-word
    // truncate at the chrome edge (dogfood: shared-pool note cut at "person").
    // Detect the included-allowance meter on the unwrapped line: wrap can
    // split "Included ... allowance:" from "% used" and would skip the bar.
    let width = content.width as usize;
    // Header chip, footer, used percent, and remaining bar share this
    // predicate. An Included pin is true even beside a Team JWT. Without
    // that pin, a Team JWT with no live personal SuperGrok session is false.
    let draws_included =
        xai_grok_shell::auth::limits_pins::next_request_draws_included_period_limits();
    let primary_bar = if draws_included {
        state.snapshot.primary.included.as_ref().map(|inc| {
            let rem = inc.remaining_fraction();
            let tone = AllowanceMeterTone::from_used_pct(inc.used_pct);
            (rem, tone)
        })
    } else {
        None
    };
    let mut display_lines: Vec<String> = Vec::new();
    let mut injected_bar = false;
    let on_credits_tab = state.window.active_tab == CREDITS_TAB;
    let mut body = if on_credits_tab {
        credits_tab_lines(state)
    } else {
        limits_tab_lines(state, now, draws_included)
    };
    // Both tabs carry the sentence when the next request does not draw
    // included SuperGrok period limits.
    if !draws_included {
        body.insert(
            0,
            "Included limits are not being drawn for the next request.".to_string(),
        );
    }
    for raw in body {
        let is_allowance_meter = is_included_allowance_used_line(&raw);
        // The fail-open printout note is required in the body text (CLI /
        // agents) before percents. Full wrap at modal width is ~15 rows and
        // buries the remaining bar under the fold. TUI shows a one-sentence
        // banner; the full note stays in content_lines.
        let display_src = if raw.as_str() == NOTE_LIMITS_PRINTOUT_NOT_USAGE {
            TUI_PRINTOUT_BANNER.to_string()
        } else if on_credits_tab {
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
    for text in display_lines
        .get(start..end)
        .expect("index out of bounds")
        .iter()
    {
        if y >= content.y + content.height {
            break;
        }
        if text.as_str() == REMAINING_BAR_SENTINEL {
            if let Some((rem, tone)) = primary_bar {
                // Indent, brackets, and the tracked cells share the content width.
                // Inner width is not capped at 34.
                let inner = content.width.saturating_sub(4);
                if inner >= 4 {
                    let fg = tone_color(tone, theme);
                    let spans =
                        progress_bar_tracked_spans(inner, rem, fg, theme.gray_dim, theme.bg_dark);
                    let mut bar_line =
                        vec![Span::styled("  [", Style::default().fg(theme.text_primary))];
                    bar_line.extend(spans);
                    bar_line.push(Span::styled("]", Style::default().fg(theme.text_primary)));
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
            joined.contains("Team prepaid remaining: not available"),
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

    #[test]
    fn remaining_bar_at_60_percent_fills_60_percent_of_the_track() {
        let reset = DateTime::parse_from_rfc3339("2026-08-08T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let bal = weekly_bal(40.0, reset);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let mut state = LimitsModalState::new(snap);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 160, 40);
        let mut painted = String::new();
        for tab in 0..2 {
            state.window.active_tab = tab;
            let mut buf = Buffer::empty(area);
            render_limits_modal(&mut buf, area, &mut state, &theme, false, now);
            for y in 0..area.height {
                for x in 0..area.width {
                    painted.push_str(buf[(x, y)].symbol());
                }
                painted.push('\n');
            }
        }
        assert!(
            painted.contains("40% used · 60% remaining"),
            "allowance line:\n{painted}"
        );
        let mut saw_wide_track = false;
        for row in painted.lines() {
            let Some(open) = row.find('[') else {
                continue;
            };
            let Some(close) = row.rfind(']') else {
                continue;
            };
            if close <= open + 1 {
                continue;
            }
            let inner: Vec<char> = row[open + 1..close].chars().collect();
            let looks_like_bar = inner
                .iter()
                .any(|glyph| matches!(*glyph, '█' | '░' | '▏' | '▎' | '▍' | '▌' | '▋' | '▊' | '▉'));
            if !looks_like_bar {
                continue;
            }
            let inner_width = inner.len();
            assert!(
                inner_width > 34,
                "track must be wider than the 34-cell cap, got {inner_width}: {row}"
            );
            let filled = inner.iter().filter(|glyph| **glyph != '░').count();
            let expected = inner_width as f64 * 0.60;
            let delta = (filled as f64 - expected).abs();
            assert!(
                delta <= 1.0,
                "filled {filled} of {inner_width} should be about 60 percent of the track: {row}"
            );
            saw_wide_track = true;
        }
        assert!(saw_wide_track, "no remaining track:\n{painted}");
    }

    #[test]
    fn limits_tab_shows_the_allowance_and_the_bar_and_credits_tab_shows_only_the_two_balances() {
        let reset = DateTime::parse_from_rfc3339("2026-08-08T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut bal = weekly_bal(40.0, reset);
        bal.prepaid_balance_cents = Some(4321);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_balance_cents(Some(8900));
        let mut state = LimitsModalState::new(snap);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 160, 40);
        let paint_tab = |state: &mut LimitsModalState, tab: usize| -> String {
            state.window.active_tab = tab;
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
        let limits = paint_tab(&mut state, LIMITS_TAB);
        assert!(
            limits.contains("40% used · 60% remaining"),
            "Limits tab allowance:\n{limits}"
        );
        assert!(
            limits.contains("Next reset:"),
            "Limits tab reset:\n{limits}"
        );
        assert!(
            limits.contains("behind a linear week") || limits.contains("ahead of a linear week"),
            "Limits tab short week line:\n{limits}"
        );
        assert!(
            limits.contains("10% behind a linear week"),
            "halfway through the week at 40% used is 10% behind a linear week:\n{limits}"
        );
        let saw_bar = limits.lines().any(|row| {
            row.contains('[') && row.contains(']') && (row.contains('█') || row.contains('░'))
        });
        assert!(saw_bar, "Limits tab bar:\n{limits}");
        assert!(
            !limits.contains("Personal credits"),
            "Limits tab must not show personal credits:\n{limits}"
        );
        assert!(
            !limits.contains("behind linear burn"),
            "Limits tab must not show the long burn note:\n{limits}"
        );
        let credits = paint_tab(&mut state, CREDITS_TAB);
        assert!(
            credits.contains("Personal credits"),
            "Credits tab personal:\n{credits}"
        );
        assert!(
            credits.contains("Console API credits"),
            "Credits tab console:\n{credits}"
        );
        assert!(
            !credits.contains("allowance:"),
            "Credits tab must not show the allowance:\n{credits}"
        );
        assert!(
            !credits.contains('█') && !credits.contains('░'),
            "Credits tab must not show the bar:\n{credits}"
        );
        for banned in [
            "Next reset:",
            "linear week",
            "behind linear burn",
            "Live sampling:",
            "Auto topup",
            "Note:",
        ] {
            assert!(
                !credits.contains(banned),
                "Credits tab is only the two balances, not {banned}:\n{credits}"
            );
        }
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
        state.window.active_tab = LIMITS_TAB;
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
        let opened = painted(&buf);
        assert!(
            opened.contains("Credits") && opened.contains("Limits"),
            "clicking the chip must open Limits and Credits tabs:\n{opened}"
        );
        let tab_line = opened
            .lines()
            .find(|line| line.contains("Limits") && line.contains("Credits"))
            .unwrap_or("");
        let limits_at = tab_line.find("Limits").expect("Limits tab label");
        let credits_at = tab_line.find("Credits").expect("Credits tab label");
        assert!(
            limits_at < credits_at,
            "Limits comes before Credits on the tab row:\n{tab_line}"
        );
        assert_eq!(
            state.window.active_tab, LIMITS_TAB,
            "limits mode opens on the Limits tab"
        );
        assert!(
            opened.contains("12% ahead of a linear week")
                || opened.contains("behind a linear week"),
            "Limits tab must show ahead of or behind a linear week:\n{opened}"
        );
        assert!(
            opened.contains("12% ahead of a linear week"),
            "half a week at 62% used is 12% ahead of a linear week:\n{opened}"
        );

        let credits_tab = state.window.tab_rects[CREDITS_TAB].expect("Credits tab rectangle");
        let outcome = modal_window::handle_modal_mouse(
            &mut state.window,
            MouseEventKind::Down(MouseButton::Left),
            credits_tab.x,
            credits_tab.y,
        );
        assert_eq!(outcome, ModalWindowOutcome::TabChanged(CREDITS_TAB));
        let pin_after_tab = std::fs::read(home.path().join("limits_pins.json")).expect("pin");
        assert_eq!(
            pin_before, pin_after_tab,
            "changing tabs must not change the spend pin"
        );

        let mut buf = Buffer::empty(modal_area);
        render_limits_modal(&mut buf, modal_area, &mut state, &theme, false, now);
        let credits_body = painted(&buf);
        assert!(
            !credits_body.contains("linear week"),
            "pacing stays off the Credits tab and off the header:\n{credits_body}"
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
                state.window.active_tab = CREDITS_TAB;
                let theme = Theme::default();
                let area = Rect::new(0, 0, 100, 40);
                let mut buf = Buffer::empty(area);
                render_limits_modal(&mut buf, area, &mut state, &theme, false, now);
                assert_eq!(
                    state.window.active_tab, CREDITS_TAB,
                    "Credits is the second tab"
                );
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
        let credits_tab = state.window.tab_rects[CREDITS_TAB].expect("Credits tab rectangle");
        let outcome = modal_window::handle_modal_mouse(
            &mut state.window,
            MouseEventKind::Down(MouseButton::Left),
            credits_tab.x,
            credits_tab.y,
        );
        assert_eq!(outcome, ModalWindowOutcome::TabChanged(CREDITS_TAB));
        let limits_tab = state.window.tab_rects[LIMITS_TAB].expect("Limits tab rectangle");
        let outcome = modal_window::handle_modal_mouse(
            &mut state.window,
            MouseEventKind::Down(MouseButton::Left),
            limits_tab.x,
            limits_tab.y,
        );
        assert_eq!(outcome, ModalWindowOutcome::TabChanged(LIMITS_TAB));
        let limits = paint(&mut state);
        assert!(limits.contains("Using limits"), "{limits}");
        assert!(limits.contains("Use credits"), "{limits}");
        assert!(
            !home.path().join("limits_pins.json").exists(),
            "changing tabs must not write the spend pin"
        );
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
        use xai_grok_sampler::AllowanceExhaustAction;
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, apply_meter_source, load_limits_pins,
        };
        use xai_grok_shell::auth::{
            LimitsSnapshotDocument, LimitsSnapshotManagement,
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
        use xai_grok_sampler::AllowanceExhaustAction;
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, apply_meter_source, load_limits_pins,
        };
        use xai_grok_shell::auth::{
            LimitsSnapshotDocument, LimitsSnapshotManagement,
            apply_billing_usage_to_session_exhaust, read_limits_snapshot_file,
            write_limits_snapshot_file,
        };
        use xai_grok_shell::sampling::SamplerConfig;

        use crate::actions::ActionRegistry;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
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
    fn card_text(buf: &Buffer, area: Rect) -> String {
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn recorded_spend_point(
        buf: &Buffer,
        agent: &crate::app::agent_view::AgentView,
        label: &str,
    ) -> (u16, u16) {
        let hits = match agent.active_modal.as_ref() {
            Some(crate::views::modal::ActiveModal::Limits { state }) => &state.window.shortcut_hits,
            _ => panic!("limits card stays open"),
        };
        let text_of = |rect: Rect| -> String {
            (0..rect.width)
                .map(|dx| buf[(rect.x.saturating_add(dx), rect.y)].symbol())
                .collect::<String>()
                .trim()
                .to_string()
        };
        let texts: Vec<String> = hits.iter().map(|hit| text_of(hit.rect)).collect();
        let hit = hits
            .iter()
            .find(|hit| hit.clickable && text_of(hit.rect) == label)
            .unwrap_or_else(|| panic!("recorded shortcut rect for {label}; painted {texts:?}"));
        (hit.rect.x, hit.rect.y)
    }

    /// A click on the painted spend button goes through the app mouse handler.
    /// Use credits selects console API credits and does not spend personal
    /// credits. Use limits selects included period limits.
    #[test]
    fn click_on_the_painted_spend_button_changes_the_next_request() {
        use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, load_limits_pins,
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
        let mut bal = weekly_bal(28.0, end);
        bal.prepaid_balance_cents = Some(personal_cents);
        assert_eq!(bal.usage_pct, 28.0, "included period limits still remain");

        let mut doc = LimitsSnapshotDocument::empty(1);
        doc.management = Some(LimitsSnapshotManagement {
            prepaid_cents: Some(console_prepaid_cents),
            ..Default::default()
        });
        write_limits_snapshot_file(home.path(), &doc).expect("console prepaid snapshot");
        assert!(
            !home.path().join("limits_pins.json").exists(),
            "this click starts with no meter pin"
        );

        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_balance_cents(Some(console_prepaid_cents));
        let _theme = crate::theme::cache::pin_theme();
        let registry = crate::actions::ActionRegistry::defaults();
        let mut agent = crate::app::agent_view::test_fixtures::make_agent();
        agent.active_modal = Some(crate::views::modal::ActiveModal::Limits {
            state: Box::new(LimitsModalState::new(snap)),
        });
        let area = Rect::new(0, 0, 100, 40);
        let paint = |agent: &mut crate::app::agent_view::AgentView| -> Buffer {
            let mut buf = Buffer::empty(area);
            agent.draw_active_modal(area, &mut buf, Theme::default(), false);
            buf
        };
        let session_primary = || SamplerConfig {
            api_key: Some(session_key.into()),
            failover_api_keys: vec![console_key.into()],
            base_url: "https://cli-chat-proxy.grok.com/v1".into(),
            model: "grok-4".into(),
            session_identity_key: Some(session_key.into()),
            failover_base_url: Some("https://api.x.ai/v1".into()),
            session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
            ..Default::default()
        };
        let deliver = |agent: &mut crate::app::agent_view::AgentView, column: u16, row: u16| {
            agent.handle_input(
                &Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                }),
                &registry,
            );
        };

        let buf = paint(&mut agent);
        let card = card_text(&buf, area);
        assert!(card.contains("Using limits"), "{card}");
        assert!(card.contains("Use credits"), "{card}");
        let (column, row) = recorded_spend_point(&buf, &agent, "Use credits");
        deliver(&mut agent, column, row);

        let pins = load_limits_pins();
        assert!(!pins.use_console, "Use credits must not set use_console");
        assert_eq!(pins.meter_source, Some(MeterSource::Console));
        assert_ne!(pins.meter_source, Some(MeterSource::DollarCredits));
        let Some(crate::views::modal::ActiveModal::Limits { state }) = agent.active_modal.as_ref()
        else {
            panic!("limits card stays open after Use credits");
        };
        assert_eq!(
            state
                .snapshot
                .primary
                .dollar_credits
                .as_ref()
                .map(|meter| meter.balance_cents),
            Some(personal_cents),
            "Use credits must not spend personal credits"
        );
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));
        assert_eq!(
            read_limits_snapshot_file(home.path())
                .and_then(|saved| saved.management)
                .and_then(|mgmt| mgmt.prepaid_cents),
            Some(console_prepaid_cents)
        );
        let mut next = session_primary();
        apply_limits_pins_to_sampler_config(&mut next);
        assert_eq!(
            next.api_key.as_deref(),
            Some(console_key),
            "Use credits makes the next request use console API credits while included period limits remain"
        );
        assert!(next.base_url.contains("api.x.ai"), "{}", next.base_url);
        assert_ne!(next.api_key.as_deref(), Some(session_key));

        let buf = paint(&mut agent);
        let using_credits = card_text(&buf, area);
        assert!(using_credits.contains("Using credits"), "{using_credits}");
        assert!(using_credits.contains("Use limits"), "{using_credits}");
        let (column, row) = recorded_spend_point(&buf, &agent, "Use limits");
        deliver(&mut agent, column, row);
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
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));
    }

    /// A left click on the painted `Use limits` words changes the card.
    ///
    /// The header chip is forced over that same rect, which is the swallow
    /// the open card must ignore. Exit, comment, revise, Esc close, and the
    /// close control do not write this pin. A hard-expired personal session
    /// beside a Team JWT still takes the click. The click writes
    /// `MeterSource::Included` and does not set `use_console`. The next
    /// request draws included SuperGrok period limits. The card shows
    /// `Using limits`. The `Use limits` control is gone. The card does not
    /// say included limits are not being drawn. The usage reading in this
    /// fixture is known, so the header shows `limits 28%`. It does not
    /// invent `limits 0%` or `$0`. A live personal session does the same.
    #[test]
    #[serial_test::serial]
    fn clicking_use_limits_on_the_limits_card_is_handled() {
        use crate::actions::ActionRegistry;
        use crate::app::actions::Action;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::app::app_view::InputOutcome;
        use crate::scrollback::render::ScratchBuffer;
        use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use xai_grok_shell::auth::limits_pins::{
            LimitsPins, MeterSource, load_limits_pins, next_request_draws_included_period_limits,
            save_limits_pins,
        };
        use xai_grok_shell::auth::{
            AuthMode, GrokAuth, clear_console_team_postpaid_cache, upsert_supergrok_session,
        };

        fn paints_limits_percent(text: &str) -> bool {
            for line in text.lines() {
                let mut rest = line;
                while let Some(idx) = rest.find("limits ") {
                    let after = &rest[idx + "limits ".len()..];
                    let digits = after.chars().take_while(|c| c.is_ascii_digit()).count();
                    if digits > 0 && after[digits..].starts_with('%') {
                        return true;
                    }
                    rest = &rest[idx + "limits ".len()..];
                }
            }
            false
        }

        fn screen_of(buf: &Buffer, area: Rect) -> String {
            (0..area.height)
                .map(|y| {
                    (0..area.width)
                        .map(|x| buf[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        fn painted(buf: &Buffer, rect: Rect) -> String {
            (0..rect.width)
                .map(|dx| buf[(rect.x.saturating_add(dx), rect.y)].symbol())
                .collect::<String>()
        }

        fn phrase_rect(buf: &Buffer, area: Rect, phrase: &str) -> Rect {
            let width = phrase.chars().count() as u16;
            for y in area.y..area.y.saturating_add(area.height) {
                let mut row = String::new();
                for x in area.x..area.x.saturating_add(area.width) {
                    row.push_str(buf[(x, y)].symbol());
                }
                if let Some(byte) = row.find(phrase) {
                    let x_off = row[..byte].chars().count() as u16;
                    return Rect {
                        x: area.x.saturating_add(x_off),
                        y,
                        width,
                        height: 1,
                    };
                }
            }
            panic!("painted {phrase} is missing");
        }

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        clear_console_team_postpaid_cache();
        std::fs::write(
            home.path().join("config.toml"),
            "[auth]\npreferred_method = \"oidc\"\nauto_use_included_limits = true\n",
        )
        .expect("preferred oidc");

        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let base = "https://auth.x.ai::use-limits-click";
        let live = chrono::Utc::now() + chrono::Duration::days(1);
        let expired = chrono::Utc::now() - chrono::Duration::days(1);
        let session = |key: &str, user_id: &str, team_id: Option<&str>, expires_at| GrokAuth {
            key: key.into(),
            auth_mode: AuthMode::Oidc,
            user_id: user_id.into(),
            principal_type: if team_id.is_some() {
                Some("Team".into())
            } else {
                Some("User".into())
            },
            principal_id: Some(user_id.into()),
            team_id: team_id.map(str::to_string),
            expires_at: Some(expires_at),
            ..GrokAuth::default()
        };
        let write_auth = |map: &std::collections::BTreeMap<String, GrokAuth>| {
            std::fs::write(
                home.path().join("auth.json"),
                serde_json::to_vec_pretty(map).expect("auth json"),
            )
            .expect("write auth");
        };
        let pin_dollars = || {
            save_limits_pins(&LimitsPins {
                stay_supergrok: false,
                use_console: false,
                meter_source: Some(MeterSource::DollarCredits),
                supergrok_identity: None,
            })
            .expect("dollar-credits pin");
        };
        let mut team_map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut team_map,
            base,
            session("tok-personal-expired", "u-personal-expired", None, expired),
        );
        team_map
            .get_mut(&format!("{base}::personal"))
            .expect("expired personal slot")
            .team_id = Some("stale-team".into());
        upsert_supergrok_session(
            &mut team_map,
            base,
            session(
                "tok-team-only",
                "u-team",
                Some("team-use-limits-click"),
                live,
            ),
        );
        write_auth(&team_map);
        pin_dollars();
        assert!(
            !next_request_draws_included_period_limits(),
            "a hard-expired personal session beside a Team JWT does not draw included SuperGrok period limits"
        );

        let registry = ActionRegistry::defaults();
        let area = Rect::new(0, 0, 140, 40);
        crate::appearance::cache::set_hide_header(false);
        let mut agent = make_agent();
        let open = |agent: &mut crate::app::agent_view::AgentView, pct: f64| {
            let mut bal = weekly_bal(pct, end);
            bal.included_usage_known = true;
            agent.plan_mode_active = false;
            agent.plan_approval_view = None;
            agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
            agent.credit_balance = Some(bal.clone());
            let snap = LimitsSnapshot::from_billing(
                Some(&bal),
                None,
                SamplingIdentityKind::SuperGrokSession,
            );
            agent.active_modal = Some(crate::views::modal::ActiveModal::Limits {
                state: Box::new(LimitsModalState::new(snap)),
            });
        };
        let draw = |agent: &mut crate::app::agent_view::AgentView| -> Buffer {
            let mut buf = Buffer::empty(area);
            let mut scratch = ScratchBuffer::new();
            agent.draw(
                area,
                &mut buf,
                &registry,
                &mut scratch,
                None,
                false,
                BannerSlotParams::none(),
                false,
                false,
                &mut Vec::new(),
                AppRenderParams::default(),
            );
            buf
        };
        let click = |agent: &mut crate::app::agent_view::AgentView, column: u16, row: u16| {
            agent.handle_input(
                &Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                }),
                &registry,
            )
        };

        open(&mut agent, 28.0);
        let buf = draw(&mut agent);
        let screen = screen_of(&buf, area);
        assert!(
            screen.contains("Use limits") && screen.contains("Using credits"),
            "the team-only card offers Use limits:\n{screen}"
        );
        assert!(
            !screen.contains("Using limits") && !paints_limits_percent(&screen),
            "a Team JWT that is not drawing included limits does not claim them:\n{screen}"
        );
        let close = match agent.active_modal.as_ref() {
            Some(crate::views::modal::ActiveModal::Limits { state }) => {
                state.window.close_button_rect.expect("close control")
            }
            _ => panic!("limits card stays open"),
        };
        let esc = phrase_rect(&buf, area, "Esc close");
        let use_limits = phrase_rect(&buf, area, "Use limits");
        assert_eq!(painted(&buf, use_limits), "Use limits");
        let recorded = match agent.active_modal.as_ref() {
            Some(crate::views::modal::ActiveModal::Limits { state }) => {
                state.use_limits_hit.expect("painted Use limits hit")
            }
            _ => panic!("limits card stays open"),
        };
        assert_eq!(recorded, use_limits);
        assert!(!recorded.intersects(close));
        assert!(!recorded.intersects(esc));
        assert!(
            agent.hit_credits.rect.is_some(),
            "full draw arms the header chip"
        );

        let closed = click(&mut agent, close.x, close.y);
        assert!(
            matches!(closed, InputOutcome::Changed),
            "the close control is its own click, got {closed:?}"
        );
        assert!(
            agent.active_modal.is_none(),
            "the close control closes the card"
        );
        assert_eq!(
            load_limits_pins().meter_source,
            Some(MeterSource::DollarCredits),
            "the close control does not run Use limits"
        );

        open(&mut agent, 28.0);
        let buf = draw(&mut agent);
        let esc = phrase_rect(&buf, area, "Esc close");
        let esc_click = click(&mut agent, esc.x, esc.y);
        assert!(
            matches!(esc_click, InputOutcome::Changed),
            "Esc close is its own click, got {esc_click:?}"
        );
        assert!(
            matches!(
                agent.active_modal,
                Some(crate::views::modal::ActiveModal::Limits { .. })
            ),
            "Esc close does not run Use limits"
        );
        assert_eq!(
            load_limits_pins().meter_source,
            Some(MeterSource::DollarCredits)
        );

        let use_limits = phrase_rect(&buf, area, "Use limits");
        let before = screen_of(&buf, area);
        assert!(
            before.contains("Use limits"),
            "the click lands on the painted Use limits control:\n{before}"
        );
        agent.hit_credits.rect = Some(use_limits);
        let outcome = click(&mut agent, use_limits.x, use_limits.y);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "a left click on the painted Use limits control is not a no-op, got {outcome:?}"
        );
        assert!(
            !matches!(outcome, InputOutcome::Action(Action::ShowLimits)),
            "Use limits is not swallowed as a header-chip click"
        );
        let pins = load_limits_pins();
        assert!(!pins.use_console);
        assert_eq!(pins.meter_source, Some(MeterSource::Included));
        assert_ne!(pins.meter_source, Some(MeterSource::Console));
        assert!(
            next_request_draws_included_period_limits(),
            "Use limits makes the next request draw included SuperGrok period limits"
        );
        assert!(
            matches!(
                agent.active_modal,
                Some(crate::views::modal::ActiveModal::Limits { .. })
            ),
            "Use limits keeps the card open"
        );
        let buf = draw(&mut agent);
        let after = screen_of(&buf, area);
        assert_ne!(
            after, before,
            "a left click on Use limits must change the team-session screen"
        );
        assert!(
            !after.contains("Use limits"),
            "the Use limits control must not still be sitting there:\n{after}"
        );
        assert!(
            !after.contains("Included limits are not being drawn for the next request."),
            "the accepted choice does not refuse included SuperGrok period limits:\n{after}"
        );
        assert!(
            after.contains("Using limits") && after.contains("limits 28%"),
            "the card shows Using limits and the header shows percent used:\n{after}"
        );
        assert!(
            !after.contains("limits 0%") && !after.contains("$0"),
            "the Team JWT click does not invent limits 0% or $0:\n{after}"
        );

        let mut personal = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut personal,
            base,
            session("tok-personal-live", "u-personal-live", None, live),
        );
        write_auth(&personal);
        pin_dollars();
        assert!(
            !next_request_draws_included_period_limits(),
            "a dollar-credits pin is not included SuperGrok period limits"
        );
        open(&mut agent, 28.0);
        let buf = draw(&mut agent);
        let use_limits = phrase_rect(&buf, area, "Use limits");
        assert_eq!(painted(&buf, use_limits), "Use limits");
        agent.hit_credits.rect = Some(use_limits);
        let outcome = click(&mut agent, use_limits.x.saturating_add(1), use_limits.y);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "a personal session click on Use limits is handled, got {outcome:?}"
        );
        let pins = load_limits_pins();
        assert_eq!(pins.meter_source, Some(MeterSource::Included));
        assert!(!pins.use_console);
        assert!(
            next_request_draws_included_period_limits(),
            "Use limits makes the next request draw included SuperGrok period limits"
        );
        let buf = draw(&mut agent);
        let personal_screen = screen_of(&buf, area);
        assert!(
            personal_screen.contains("limits 28%") && personal_screen.contains("Using limits"),
            "the header shows percent used after Use limits:\n{personal_screen}"
        );
        assert!(
            !personal_screen.contains("limits 0%") && !personal_screen.contains("$0"),
            "the personal header does not invent limits 0% or $0:\n{personal_screen}"
        );
        clear_console_team_postpaid_cache();
    }

    /// Owed outcome: start from `MeterSource::Included` beside a Team JWT.
    /// Click the credits control. The footer paints `Using credits`.
    /// `use_console` stays false. The pin is not `MeterSource::Console`.
    /// The click switches the next request off included SuperGrok period
    /// limits. SuperGrok is paid.
    #[test]
    #[serial_test::serial]
    fn clicking_using_credits_while_using_limits_is_selected_switches_off_included_period_limits_and_paints_using_credits()
     {
        use crate::actions::ActionRegistry;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::app::app_view::InputOutcome;
        use crate::scrollback::render::ScratchBuffer;
        use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use xai_grok_shell::auth::limits_pins::{
            LimitsPins, MeterSource, load_limits_pins, next_request_draws_included_period_limits,
            save_limits_pins,
        };
        use xai_grok_shell::auth::{
            AuthMode, GrokAuth, clear_console_team_postpaid_cache, upsert_supergrok_session,
        };

        fn screen_of(buf: &Buffer, area: Rect) -> String {
            (0..area.height)
                .map(|y| {
                    (0..area.width)
                        .map(|x| buf[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        clear_console_team_postpaid_cache();
        std::fs::write(
            home.path().join("config.toml"),
            "[auth]\npreferred_method = \"oidc\"\nauto_use_included_limits = true\n",
        )
        .expect("preferred oidc");

        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let base = "https://auth.x.ai::using-credits-click";
        let live = chrono::Utc::now() + chrono::Duration::days(1);
        let expired = chrono::Utc::now() - chrono::Duration::days(1);
        let session = |key: &str, user_id: &str, team_id: Option<&str>, expires_at| GrokAuth {
            key: key.into(),
            auth_mode: AuthMode::Oidc,
            user_id: user_id.into(),
            principal_type: if team_id.is_some() {
                Some("Team".into())
            } else {
                Some("User".into())
            },
            principal_id: Some(user_id.into()),
            team_id: team_id.map(str::to_string),
            expires_at: Some(expires_at),
            ..GrokAuth::default()
        };
        let mut team_map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut team_map,
            base,
            session("tok-personal-expired", "u-personal-expired", None, expired),
        );
        team_map
            .get_mut(&format!("{base}::personal"))
            .expect("expired personal slot")
            .team_id = Some("stale-team".into());
        upsert_supergrok_session(
            &mut team_map,
            base,
            session(
                "tok-team-only",
                "u-team",
                Some("team-using-credits-click"),
                live,
            ),
        );
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec_pretty(&team_map).expect("auth json"),
        )
        .expect("write auth");
        save_limits_pins(&LimitsPins {
            stay_supergrok: false,
            use_console: false,
            meter_source: Some(MeterSource::Included),
            supergrok_identity: None,
        })
        .expect("Included pin");
        assert!(
            next_request_draws_included_period_limits(),
            "Using limits is already selected beside the Team JWT"
        );
        assert!(!load_limits_pins().use_console);

        let registry = ActionRegistry::defaults();
        let area = Rect::new(0, 0, 140, 40);
        crate::appearance::cache::set_hide_header(false);
        let mut agent = make_agent();
        let mut bal = weekly_bal(28.0, end);
        bal.included_usage_known = true;
        agent.plan_mode_active = false;
        agent.plan_approval_view = None;
        agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
        agent.credit_balance = Some(bal.clone());
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        assert!(
            snap.console.balance_cents.is_none(),
            "this click has no console team prepaid balance"
        );
        agent.active_modal = Some(crate::views::modal::ActiveModal::Limits {
            state: Box::new(LimitsModalState::new(snap)),
        });
        let mut buf = Buffer::empty(area);
        let mut scratch = ScratchBuffer::new();
        agent.draw(
            area,
            &mut buf,
            &registry,
            &mut scratch,
            None,
            false,
            BannerSlotParams::none(),
            false,
            false,
            &mut Vec::new(),
            AppRenderParams::default(),
        );
        let before = screen_of(&buf, area);
        assert!(
            before.contains("Using limits") && before.contains("Use credits"),
            "Using limits is selected, so the credits control is Use credits:\n{before}"
        );
        assert!(
            !before.contains("Using credits"),
            "the credits choice is not already accepted:\n{before}"
        );
        let (column, row) = recorded_spend_point(&buf, &agent, "Use credits");
        agent.hit_credits.rect = Some(Rect {
            x: column,
            y: row,
            width: 1,
            height: 1,
        });
        let outcome = agent.handle_input(
            &Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            }),
            &registry,
        );
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "a left click on Use credits is handled, got {outcome:?}"
        );
        let pins = load_limits_pins();
        assert!(!pins.use_console, "Using credits keeps use_console false");
        assert_ne!(
            pins.meter_source,
            Some(MeterSource::Console),
            "Using credits does not pin console team prepaid / console API credits"
        );
        assert!(
            !next_request_draws_included_period_limits(),
            "Using credits switches the next request off included SuperGrok period limits"
        );
        let mut buf = Buffer::empty(area);
        let mut scratch = ScratchBuffer::new();
        agent.draw(
            area,
            &mut buf,
            &registry,
            &mut scratch,
            None,
            false,
            BannerSlotParams::none(),
            false,
            false,
            &mut Vec::new(),
            AppRenderParams::default(),
        );
        let after = screen_of(&buf, area);
        assert!(
            after.contains("Using credits"),
            "the footer paints Using credits:\n{after}"
        );
        clear_console_team_postpaid_cache();
    }

    /// No pin file. The card says `Using limits`. The next request uses
    /// included period limits. A meter pin that is already set stays set.
    #[test]
    fn missing_meter_pin_defaults_to_included_limits() {
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, apply_meter_source, load_limits_pins,
        };
        use xai_grok_shell::auth::{
            LimitsSnapshotDocument, LimitsSnapshotManagement, write_limits_snapshot_file,
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

        let mut doc = LimitsSnapshotDocument::empty(1);
        doc.management = Some(LimitsSnapshotManagement {
            prepaid_cents: Some(console_prepaid_cents),
            ..Default::default()
        });
        write_limits_snapshot_file(home.path(), &doc).expect("console prepaid snapshot");
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_balance_cents(Some(console_prepaid_cents));
        assert_eq!(
            snap.console.balance_cents,
            Some(console_prepaid_cents),
            "console API credits are available, so the default is not an empty balance"
        );

        let area = Rect::new(0, 0, 100, 40);
        let paint = |state: &mut LimitsModalState| -> String {
            let theme = Theme::default();
            let mut buf = Buffer::empty(area);
            render_limits_modal(&mut buf, area, state, &theme, false, now);
            card_text(&buf, area)
        };
        let session_primary = || SamplerConfig {
            api_key: Some(session_key.into()),
            failover_api_keys: vec![console_key.into()],
            base_url: "https://cli-chat-proxy.grok.com/v1".into(),
            model: "grok-4".into(),
            session_identity_key: Some(session_key.into()),
            failover_base_url: Some("https://api.x.ai/v1".into()),
            session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
            ..Default::default()
        };
        let console_primary = || SamplerConfig {
            api_key: Some(console_key.into()),
            failover_api_keys: vec![session_key.into()],
            base_url: "https://api.x.ai/v1".into(),
            model: "grok-4".into(),
            session_identity_key: Some(session_key.into()),
            failover_base_url: Some("https://api.x.ai/v1".into()),
            session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
            ..Default::default()
        };

        apply_meter_source(MeterSource::Console).expect("existing meter pin");
        let pinned = load_limits_pins();
        assert_eq!(pinned.meter_source, Some(MeterSource::Console));
        assert!(!pinned.use_console);
        let mut state = LimitsModalState::new(snap);
        let _while_pinned = paint(&mut state);
        let mut honored = session_primary();
        apply_limits_pins_to_sampler_config(&mut honored);
        assert_eq!(honored.api_key.as_deref(), Some(console_key));
        assert_eq!(
            load_limits_pins(),
            pinned,
            "a meter pin that is already set must not be cleared"
        );

        std::fs::remove_file(home.path().join("limits_pins.json")).expect("no pin file");
        assert_eq!(load_limits_pins().meter_source, None);
        let card = paint(&mut state);
        assert!(card.contains("Using limits"), "{card}");
        assert!(!card.contains("Using credits"), "{card}");
        let mut next = console_primary();
        apply_limits_pins_to_sampler_config(&mut next);
        assert_eq!(
            next.api_key.as_deref(),
            Some(session_key),
            "a missing meter pin uses included period limits for the next request"
        );
        assert!(
            next.base_url.contains("cli-chat-proxy"),
            "{}",
            next.base_url
        );
        assert_ne!(next.api_key.as_deref(), Some(console_key));
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));
        assert!(!load_limits_pins().use_console);
        assert_ne!(
            load_limits_pins().meter_source,
            Some(MeterSource::DollarCredits)
        );
    }

    /// Named contract: console API credits balance and spend use the console
    /// inference key. No management key. A fixture of 90035 cents paints
    /// Console API credits and $900.35, and Use credits sends that inference
    /// key. Personal cents stay unchanged.
    #[tokio::test]
    #[serial_test::serial]
    async fn console_api_credits_balance_and_spend_without_a_management_key() {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Mutex};
        use xai_grok_shell::auth::credentials_store::FORCE_FILE_ENV;
        use xai_grok_shell::auth::limits_pins::{
            apply_limits_pins_to_sampler_config, load_limits_pins,
        };
        use xai_grok_shell::auth::{
            LimitsSnapshotDocument, clear_console_team_prepaid_cache,
            fetch_management_into_snapshot, write_limits_snapshot_file,
        };
        use xai_grok_shell::sampling::SamplerConfig;
        use xai_grok_test_support::EnvGuard;

        let inference_key = "inference-key-90035";
        let session_key = "included-period-session-token";
        let personal_cents = 4321_i64;
        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _home = EnvGuard::set("GROK_HOME", home.path());
        let _force = EnvGuard::set(FORCE_FILE_ENV, "1");
        let _mgmt = EnvGuard::unset("XAI_MANAGEMENT_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _team = EnvGuard::unset("XAI_MANAGEMENT_TEAM_ID");
        let _key = EnvGuard::set("XAI_API_KEY", inference_key);
        clear_console_team_prepaid_cache();

        let stop = Arc::new(AtomicBool::new(false));
        let paths = Arc::new(Mutex::new(Vec::<String>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind fixture");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("port").port();
        let stop_thread = Arc::clone(&stop);
        let paths_thread = Arc::clone(&paths);
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while !stop_thread.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                        let mut buf = Vec::new();
                        let mut tmp = [0u8; 2048];
                        let read_deadline =
                            std::time::Instant::now() + std::time::Duration::from_secs(2);
                        while std::time::Instant::now() < read_deadline {
                            match stream.read(&mut tmp) {
                                Ok(0) => break,
                                Ok(n) => {
                                    buf.extend_from_slice(&tmp[..n]);
                                    if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        let req = String::from_utf8_lossy(&buf);
                        let first = req.lines().next().unwrap_or("").to_string();
                        if let Ok(mut seen) = paths_thread.lock() {
                            seen.push(first.clone());
                        }
                        let authed = req
                            .to_ascii_lowercase()
                            .contains(&format!("bearer {inference_key}"));
                        let (status, body) = if !authed {
                            ("401 Unauthorized", r#"{"error":"unauthorized"}"#)
                        } else if first.contains("GET /v1/api-key ") {
                            (
                                "200 OK",
                                r#"{"team_id":"team-inference-90035","api_key_blocked":false}"#,
                            )
                        } else if first
                            .contains("GET /v1/billing/teams/team-inference-90035/prepaid/balance ")
                        {
                            ("200 OK", r#"{"total":{"val":"90035"},"changes":[]}"#)
                        } else {
                            ("404 Not Found", r#"{"error":"not found"}"#)
                        };
                        let resp = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        let base = format!("http://127.0.0.1:{port}/v1");
        let _base = EnvGuard::set("GROK_XAI_API_BASE_URL", &base);

        let fetched = fetch_management_into_snapshot().await;
        let cents = fetched.as_ref().and_then(|mgmt| mgmt.prepaid_cents);
        assert_eq!(
            cents,
            Some(90035),
            "console API credits must be read with the inference key, not a management key"
        );
        let seen = paths
            .lock()
            .expect("paths")
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            seen.contains("GET /v1/api-key "),
            "balance read must ask api.x.ai for the team id: {seen}"
        );
        assert!(
            seen.contains("/prepaid/balance"),
            "balance read must use the inference host: {seen}"
        );
        assert!(
            !seen.contains("management-api"),
            "must not call management-api.x.ai for console API credits: {seen}"
        );

        let mut doc = LimitsSnapshotDocument::empty(1);
        doc.management = fetched;
        write_limits_snapshot_file(home.path(), &doc).expect("write fetched balance");

        let end = DateTime::parse_from_rfc3339("2026-08-08T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut bal = weekly_bal(28.0, end);
        bal.prepaid_balance_cents = Some(personal_cents);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_balance_cents(cents)
                .with_console_key_available(true);
        let mut state = LimitsModalState::new(snap);
        state.window.active_tab = CREDITS_TAB;
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
        assert!(
            painted.contains("Console API credits"),
            "Credits tab names console API credits: {painted}"
        );
        assert!(
            painted.contains("$900.35"),
            "90035 cents is $900.35: {painted}"
        );
        assert!(
            !painted.contains("no management key"),
            "Credits tab must not say no management key when the balance is known: {painted}"
        );

        let hit = state
            .window
            .shortcut_hits
            .iter()
            .find(|hit| hit.clickable && hit.id == SPEND_OTHER_CHOICE_ID)
            .map(|hit| hit.rect)
            .expect("Use credits is a button");
        let outcome = handle_limits_mouse(
            &mut state,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            hit.x,
            hit.y,
        );
        assert_eq!(outcome, LimitsModalOutcome::Changed);
        let mut next = SamplerConfig {
            api_key: Some(session_key.into()),
            failover_api_keys: Vec::new(),
            base_url: "https://cli-chat-proxy.grok.com/v1".into(),
            model: "grok-4".into(),
            session_identity_key: Some(session_key.into()),
            failover_base_url: Some("https://api.x.ai/v1".into()),
            session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
            ..Default::default()
        };
        apply_limits_pins_to_sampler_config(&mut next);
        assert_eq!(
            next.api_key.as_deref(),
            Some(inference_key),
            "Use credits sets the next request to the console inference key"
        );
        assert_ne!(next.api_key.as_deref(), Some(session_key));
        assert!(next.base_url.contains("api.x.ai"), "{}", next.base_url);
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));
        assert!(!load_limits_pins().use_console);

        let gap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_key_available(true)
                .with_console_prepaid_gap(ConsoleTeamPrepaidGap::MissingManagementKey);
        let mut gap_state = LimitsModalState::new(gap);
        gap_state.window.active_tab = CREDITS_TAB;
        let mut gap_buf = Buffer::empty(area);
        render_limits_modal(&mut gap_buf, area, &mut gap_state, &theme, false, now);
        let gap_paint = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| gap_buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !gap_paint.contains("no management key"),
            "an inference key must not paint no management key: {gap_paint}"
        );
        assert!(
            gap_paint.contains("Console API credits: not available"),
            "no balance yet is not available, not an invented dollar amount: {gap_paint}"
        );

        stop.store(true, Ordering::SeqCst);
        let _ = server.join();
    }
    /// Named contract: no management key in the environment or the secret store.
    /// A console inference key is present. A fixture billing response has 76674
    /// cents. The Credits tab contains `Console API credits` and `$766.74`. It
    /// does not contain `no management key`. The footer does not contain
    /// `Using limits`, because this fixture is the team credit balance the
    /// requests are spending, not included SuperGrok period limits.
    #[tokio::test]
    #[serial_test::serial]
    async fn credits_tab_shows_team_billing_balance_without_a_management_key() {
        use std::io::{Read, Write};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        use xai_grok_shell::auth::credentials_store::{CredentialsStore, FORCE_FILE_ENV};
        use xai_grok_shell::auth::{
            clear_management_api_key, fetch_management_into_snapshot,
            load_stored_management_api_key, management_api_key_from_env,
        };
        use xai_grok_test_support::EnvGuard;

        let inference_key = "inference-key-76674";
        let team_id = "team-inference-76674";
        let team_cents: i64 = 76674;
        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _home = EnvGuard::set("GROK_HOME", home.path());
        let _force = EnvGuard::set(FORCE_FILE_ENV, "1");
        let _mgmt = EnvGuard::unset("XAI_MANAGEMENT_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _team = EnvGuard::unset("XAI_MANAGEMENT_TEAM_ID");
        let _key = EnvGuard::set("XAI_API_KEY", inference_key);
        let store = CredentialsStore::default_store();
        clear_management_api_key(&store).expect("clear management key from the secret store");
        assert!(
            management_api_key_from_env().is_none(),
            "the environment must not hold a management key"
        );
        assert!(
            load_stored_management_api_key(&store)
                .expect("read secret store")
                .is_none(),
            "the secret store must not hold a management key"
        );

        let api_key_body = format!(r#"{{"team_id":"{team_id}","api_key_blocked":false}}"#);
        let balance_body = format!(r#"{{"total":{{"val":"{team_cents}"}},"changes":[]}}"#);
        let stop = Arc::new(AtomicBool::new(false));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind fixture");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("port").port();
        let stop_thread = Arc::clone(&stop);
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while !stop_thread.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                        let mut buf = Vec::new();
                        let mut tmp = [0u8; 2048];
                        let read_deadline =
                            std::time::Instant::now() + std::time::Duration::from_secs(2);
                        while std::time::Instant::now() < read_deadline {
                            match stream.read(&mut tmp) {
                                Ok(0) => break,
                                Ok(n) => {
                                    buf.extend_from_slice(&tmp[..n]);
                                    if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        let req = String::from_utf8_lossy(&buf);
                        let first = req.lines().next().unwrap_or("").to_string();
                        let authed = req
                            .to_ascii_lowercase()
                            .contains(&format!("bearer {inference_key}"));
                        let balance_path =
                            format!("GET /v1/billing/teams/{team_id}/prepaid/balance ");
                        let (status, body) = if !authed {
                            (
                                "401 Unauthorized",
                                r#"{"error":"unauthorized"}"#.to_string(),
                            )
                        } else if first.contains("GET /v1/api-key ") {
                            ("200 OK", api_key_body.clone())
                        } else if first.contains(&balance_path) {
                            ("200 OK", balance_body.clone())
                        } else {
                            ("404 Not Found", r#"{"error":"not found"}"#.to_string())
                        };
                        let resp = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        let base = format!("http://127.0.0.1:{port}/v1");
        let _base = EnvGuard::set("GROK_XAI_API_BASE_URL", &base);
        // The fixture is the billing response. The live card below is the
        // snapshot `/limits` opens. It does not copy these cents on.
        let _fetched = fetch_management_into_snapshot().await;

        let end = DateTime::parse_from_rfc3339("2026-08-08T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut bal = weekly_bal(28.0, end);
        bal.prepaid_balance_cents = None;
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let mut state = LimitsModalState::new(snap);
        state.window.active_tab = CREDITS_TAB;
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
        assert!(
            !painted.contains("no management key"),
            "the live card paints no management key from MissingManagementKey:\n{painted}"
        );
        assert!(
            painted.contains("Console API credits"),
            "Credits tab names console API credits: {painted}"
        );
        assert!(
            painted.contains("$766.74"),
            "76674 cents is $766.74: {painted}"
        );
        assert!(
            !painted.contains("Using limits"),
            "the footer must not say Using limits when the requests are spending this team credit balance, not included SuperGrok period limits:\n{painted}"
        );

        stop.store(true, Ordering::SeqCst);
        let _ = server.join();
    }

    /// `stay_supergrok` is set, `use_console` is not, and the meter pin is
    /// absent. `[auth] preferred_method` is `oidc`. The SuperGrok session is a
    /// bearer, and `api_key` is a console key. After
    /// `apply_limits_pins_to_sampler_config`, the painted footer may say
    /// `Using limits` only when the next request is that SuperGrok session.
    /// It must not say `Using limits` when the next request is still the
    /// console key.
    #[test]
    #[serial_test::serial]
    fn stay_supergrok_with_oidc_does_not_say_using_limits_unless_the_request_is_the_supergrok_session()
     {
        use xai_grok_sampler::BearerResolver;
        use xai_grok_shell::auth::limits_pins::{
            LimitsPins, apply_limits_pins_to_sampler_config, load_limits_pins, save_limits_pins,
        };
        use xai_grok_shell::sampling::SamplerConfig;
        use xai_grok_test_support::EnvGuard;

        #[derive(Debug)]
        struct SessionBearer;
        impl BearerResolver for SessionBearer {
            fn current_bearer(&self) -> Option<String> {
                Some("included-period-session-token".to_owned())
            }
        }

        let console_key = "console-api-credits-key";
        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _home = EnvGuard::set("GROK_HOME", home.path());
        std::fs::write(
            home.path().join("config.toml"),
            "[auth]\npreferred_method = \"oidc\"\n",
        )
        .expect("write preferred_method oidc");
        save_limits_pins(&LimitsPins {
            stay_supergrok: true,
            use_console: false,
            meter_source: None,
            supergrok_identity: None,
        })
        .expect("stay_supergrok pin");
        let pins = load_limits_pins();
        assert!(pins.stay_supergrok);
        assert!(!pins.use_console);
        assert!(pins.meter_source.is_none());

        let session_bearer: xai_grok_sampler::SharedBearerResolver =
            std::sync::Arc::new(SessionBearer);
        let mut sampling = SamplerConfig {
            api_key: Some(console_key.to_owned()),
            failover_api_keys: Vec::new(),
            base_url: "https://api.x.ai/v1".into(),
            model: "grok-4".into(),
            session_identity_key: None,
            bearer_resolver: None,
            session_bearer_resolver: Some(session_bearer),
            failover_base_url: Some("https://api.x.ai/v1".into()),
            session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
            ..Default::default()
        };
        assert!(sampling.bearer_resolver.is_none());
        assert!(sampling.session_bearer_resolver.is_some());
        assert_eq!(sampling.api_key.as_deref(), Some(console_key));

        apply_limits_pins_to_sampler_config(&mut sampling);

        let active = sampling.api_key.as_deref().unwrap_or("").trim();
        let session_on_config = sampling
            .session_identity_key
            .as_deref()
            .unwrap_or("")
            .trim();
        let on_session = sampling.bearer_resolver.is_some()
            || (!session_on_config.is_empty() && active == session_on_config);
        let on_console = sampling.bearer_resolver.is_none()
            && !active.is_empty()
            && active == console_key
            && active != session_on_config;
        assert!(
            on_console || on_session,
            "after apply, the next request is the console key or the SuperGrok session; api_key={active:?}"
        );

        let end = DateTime::parse_from_rfc3339("2026-08-08T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut bal = weekly_bal(28.0, end);
        bal.prepaid_balance_cents = Some(4321);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_balance_cents(Some(8900));
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
        if on_console {
            assert!(
                !painted.contains("Using limits"),
                "the next request stayed on the console key, so the footer must not say Using limits:\n{painted}"
            );
        }
    }

    /// The limits label is absent when included SuperGrok period limits are
    /// not the meter in use. Limits mode defaults to limits enabled. Limits
    /// is ordered before Credits.
    #[test]
    fn limits_label_is_absent_unless_that_meter_is_in_use_and_limits_mode_defaults_enabled_and_limits_come_before_credits()
     {
        use crate::actions::ActionRegistry;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::scrollback::render::ScratchBuffer;
        use xai_grok_shell::auth::limits_pins::{
            LimitsPins, MeterSource, load_limits_pins, next_request_draws_included_period_limits,
            save_limits_pins,
        };

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
        // Safety: restored by EnvGuard.
        unsafe {
            std::env::set_var("GROK_HOME", home.path());
        }

        assert!(
            xai_grok_shell::auth::limits_pins::limits_mode_enables_included_period_limits_by_default(),
            "when the mode is limits, included SuperGrok period limits are enabled by default"
        );
        assert!(
            xai_grok_shell::auth::default_auto_use_included_limits(),
            "limits mode keeps auto_use_included_limits on by default"
        );
        assert_eq!(load_limits_pins(), LimitsPins::default());
        assert!(
            next_request_draws_included_period_limits(),
            "a fresh home draws included SuperGrok period limits"
        );
        assert_eq!(CARD_TABS[0], "Limits");
        assert_eq!(CARD_TABS[1], "Credits");

        let end = DateTime::parse_from_rfc3339("2026-08-08T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let balance = weekly_bal(28.0, end);

        let paint_header = |balance: &CreditBalance| -> String {
            crate::appearance::cache::set_hide_header(false);
            let mut agent = make_agent();
            agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
            agent.credit_balance = Some(balance.clone());
            let area = Rect::new(0, 0, 120, 40);
            let mut header = ratatui::buffer::Buffer::empty(area);
            let mut scratch = ScratchBuffer::new();
            agent.draw(
                area,
                &mut header,
                &ActionRegistry::defaults(),
                &mut scratch,
                None,
                false,
                BannerSlotParams::none(),
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
        };
        let header = paint_header(&balance);
        let row = header
            .lines()
            .find(|line| line.contains("limits 28%"))
            .unwrap_or("")
            .to_string();
        assert!(
            row.contains("limits 28%"),
            "limits mode paints the short limits label when that meter is in use:\n{header}"
        );

        let snap = LimitsSnapshot::from_billing(
            Some(&balance),
            None,
            SamplingIdentityKind::SuperGrokSession,
        );
        let mut state = LimitsModalState::new(snap);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 100, 40);
        let paint_card = |state: &mut LimitsModalState| -> String {
            let mut buf = ratatui::buffer::Buffer::empty(area);
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
        let card = paint_card(&mut state);
        assert_eq!(state.window.active_tab, LIMITS_TAB);
        assert!(
            card.contains("Using limits"),
            "limits mode says Using limits:\n{card}"
        );
        let tab_line = card
            .lines()
            .find(|line| line.contains("Limits") && line.contains("Credits"))
            .unwrap_or("");
        let limits_at = tab_line.find("Limits").expect("Limits tab");
        let credits_at = tab_line.find("Credits").expect("Credits tab");
        assert!(
            limits_at < credits_at,
            "Limits is ordered before Credits:\n{tab_line}"
        );

        save_limits_pins(&LimitsPins {
            meter_source: Some(MeterSource::DollarCredits),
            ..LimitsPins::default()
        })
        .expect("dollar credits pin");
        assert!(
            !next_request_draws_included_period_limits(),
            "SuperGrok dollar credits are not included SuperGrok period limits"
        );
        let dollar_header = paint_header(&balance);
        let dollar_row = dollar_header
            .lines()
            .find(|line| line.contains("/tmp"))
            .unwrap_or("");
        assert!(
            !dollar_row.contains("limits"),
            "the limits label is absent when SuperGrok dollar credits are the meter:\n{dollar_row}"
        );
        let dollar_card = paint_card(&mut state);
        assert!(
            !dollar_card.contains("Using limits"),
            "the card must not say Using limits when the meter is SuperGrok dollar credits:\n{dollar_card}"
        );
        let dollar_tabs = dollar_card
            .lines()
            .find(|line| line.contains("Limits") && line.contains("Credits"))
            .unwrap_or("");
        assert!(
            dollar_tabs.find("Limits").unwrap_or(usize::MAX)
                < dollar_tabs.find("Credits").unwrap_or(0),
            "Limits stays before Credits when the meter is SuperGrok dollar credits:\n{dollar_tabs}"
        );

        std::fs::write(
            home.path().join("config.toml"),
            "[auth]\npreferred_method = \"api_key\"\n",
        )
        .expect("preferred_method api_key");
        save_limits_pins(&LimitsPins {
            stay_supergrok: true,
            use_console: false,
            meter_source: None,
            supergrok_identity: None,
        })
        .expect("stay pin");
        assert!(
            !next_request_draws_included_period_limits(),
            "stay-supergrok with preferred_method api_key keeps the console key"
        );
        let console_header = paint_header(&balance);
        let console_row = console_header
            .lines()
            .find(|line| line.contains("/tmp"))
            .unwrap_or("");
        assert!(
            !console_row.contains("limits"),
            "the limits label is absent when the next request is the console key:\n{console_row}"
        );
        let console_card = paint_card(&mut state);
        assert!(
            !console_card.contains("Using limits"),
            "the card must not say Using limits when the next request is the console key:\n{console_card}"
        );
    }

    /// Header `limits 0%` and footer `Using limits` are absent when the card
    /// says included limits are not being drawn. An unread meter is
    /// `not available yet`, which is not a live 0%.
    #[test]
    #[serial_test::serial]
    fn limits_label_is_absent_when_the_card_says_included_limits_are_not_being_drawn_and_an_unread_meter_is_not_zero_percent()
     {
        use crate::actions::ActionRegistry;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::scrollback::render::ScratchBuffer;
        use xai_grok_shell::auth::limits_pins::{
            LimitsPins, MeterSource, next_request_draws_included_period_limits, save_limits_pins,
        };
        use xai_grok_test_support::EnvGuard;

        const NOT_DRAWN: &str = "Included limits are not being drawn for the next request.";
        const NOT_READ: &str = "Included weekly allowance: not available yet";

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = EnvGuard::set("GROK_HOME", home.path());

        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-10-06T22:08:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let known = weekly_bal(28.0, end);
        let mut unread = weekly_bal(0.0, end);
        unread.included_usage_known = false;
        unread.usage_pct = 0.0;
        unread.period_end_display = Some("10-12 06:59 UTC".to_string());

        let paint_header = |balance: &CreditBalance| -> String {
            crate::appearance::cache::set_hide_header(false);
            let mut agent = make_agent();
            agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
            agent.credit_balance = Some(balance.clone());
            let area = Rect::new(0, 0, 140, 40);
            let mut header = ratatui::buffer::Buffer::empty(area);
            let mut scratch = ScratchBuffer::new();
            agent.draw(
                area,
                &mut header,
                &ActionRegistry::defaults(),
                &mut scratch,
                None,
                false,
                BannerSlotParams::none(),
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
        };
        let theme = Theme::default();
        let area = Rect::new(0, 0, 140, 40);
        let paint_card = |state: &mut LimitsModalState| -> String {
            let mut buf = ratatui::buffer::Buffer::empty(area);
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

        assert!(next_request_draws_included_period_limits());
        let drawing_header = paint_header(&known);
        assert!(
            drawing_header.contains("limits 28%"),
            "limits mode paints the short limits label when that meter is in use:\n{drawing_header}"
        );
        assert!(
            !drawing_header.contains("SuperGrok period"),
            "the header keeps the short limits label:\n{drawing_header}"
        );
        let drawing_snap = LimitsSnapshot::from_billing(
            Some(&known),
            None,
            SamplingIdentityKind::SuperGrokSession,
        );
        let mut drawing = LimitsModalState::new(drawing_snap);
        let drawing_shortcuts = spend_shortcuts(&drawing);
        assert_eq!(drawing_shortcuts.len(), 3);
        assert_eq!(drawing_shortcuts[0].label, "Using limits");
        assert_eq!(drawing_shortcuts[1].label, "Use credits");
        assert_eq!(drawing_shortcuts[2].label, "Esc close");
        let drawing_card = paint_card(&mut drawing);
        assert!(
            drawing_card.contains("Using limits"),
            "limits mode says Using limits when the next request draws included SuperGrok period limits:\n{drawing_card}"
        );
        assert!(
            !drawing_card.contains(NOT_DRAWN),
            "a request that draws included SuperGrok period limits does not say they are not being drawn:\n{drawing_card}"
        );
        let drawing_tabs = drawing_card
            .lines()
            .find(|line| line.contains("Limits") && line.contains("Credits"))
            .unwrap_or("");
        assert!(
            drawing_tabs.find("Limits").unwrap_or(usize::MAX)
                < drawing_tabs.find("Credits").unwrap_or(0),
            "Limits is ordered before Credits:\n{drawing_tabs}"
        );

        let unread_header = paint_header(&unread);
        assert!(
            !unread_header.contains("limits 0%"),
            "an unread meter does not paint limits 0%:\n{unread_header}"
        );
        assert!(
            !unread_header.contains("limits "),
            "an unread meter does not paint a limits chip:\n{unread_header}"
        );
        let unread_snap = LimitsSnapshot::from_billing(
            Some(&unread),
            None,
            SamplingIdentityKind::SuperGrokSession,
        );
        let mut unread_state = LimitsModalState::new(unread_snap);
        assert_eq!(unread_state.window.active_tab, LIMITS_TAB);
        let unread_card = paint_card(&mut unread_state);
        assert!(
            unread_card.contains(NOT_READ),
            "an unread included meter says not available yet:\n{unread_card}"
        );
        assert!(
            !unread_card.contains("0% used"),
            "not available yet is not a live 0%:\n{unread_card}"
        );
        assert!(
            !unread_card.contains("SuperGrok limits 0% used"),
            "an unread meter does not paint SuperGrok limits 0% used:\n{unread_card}"
        );
        assert!(
            !unread_card.contains("resets 10-12 06:59 UTC"),
            "an unread meter does not invent a reset title:\n{unread_card}"
        );
        assert!(
            !unread_card.contains(NOT_DRAWN),
            "limits mode still draws included SuperGrok period limits when the meter is unread:\n{unread_card}"
        );
        assert!(
            unread_card.contains("Using limits"),
            "the footer says Using limits when the next request draws that meter:\n{unread_card}"
        );
        assert!(
            !unread_card.contains("free") && !unread_card.contains("extras"),
            "SuperGrok stays a paid product with distinct meters:\n{unread_card}"
        );

        save_limits_pins(&LimitsPins {
            meter_source: Some(MeterSource::DollarCredits),
            ..LimitsPins::default()
        })
        .expect("dollar credits pin");
        assert!(!next_request_draws_included_period_limits());
        let stopped_header = paint_header(&unread);
        assert!(
            !stopped_header.contains("limits 0%"),
            "the header must not say limits 0% when included limits are not being drawn:\n{stopped_header}"
        );
        assert!(
            !stopped_header.contains("limits "),
            "the header chip stays off when the next request does not draw included SuperGrok period limits:\n{stopped_header}"
        );
        let stopped_snap = LimitsSnapshot::from_billing(
            Some(&unread),
            None,
            SamplingIdentityKind::SuperGrokSession,
        )
        .with_console_balance_cents(None)
        .with_console_prepaid_gap(ConsoleTeamPrepaidGap::Unavailable);
        let mut stopped = LimitsModalState::new(stopped_snap);
        stopped.window.active_tab = CREDITS_TAB;
        let stopped_shortcuts = spend_shortcuts(&stopped);
        assert_eq!(stopped_shortcuts.len(), 3);
        assert_eq!(stopped_shortcuts[0].label, "Use limits");
        assert_eq!(stopped_shortcuts[1].label, "Using credits");
        assert_eq!(stopped_shortcuts[2].label, "Esc close");
        let credits_card = paint_card(&mut stopped);
        assert_eq!(stopped.window.active_tab, CREDITS_TAB);
        assert!(
            credits_card.contains(NOT_DRAWN),
            "the Credits tab says included limits are not being drawn for the next request:\n{credits_card}"
        );
        assert!(
            credits_card.contains("Console API credits: not available"),
            "console API credits stay not available when that meter has no balance:\n{credits_card}"
        );
        assert!(
            !credits_card.contains("Using limits"),
            "the footer must not say Using limits when the card says included limits are not being drawn:\n{credits_card}"
        );
        assert!(
            !credits_card.contains("limits 0%"),
            "the card must not say limits 0% beside the not-drawn sentence:\n{credits_card}"
        );
        assert!(
            !credits_card.contains("0% used"),
            "the card must not invent 0% used beside the not-drawn sentence:\n{credits_card}"
        );
        assert!(
            !credits_card.contains("SuperGrok limits 0% used"),
            "the card must not paint SuperGrok limits 0% used:\n{credits_card}"
        );
        let credits_tabs = credits_card
            .lines()
            .find(|line| line.contains("Limits") && line.contains("Credits"))
            .unwrap_or("");
        assert!(
            credits_tabs.find("Limits").unwrap_or(usize::MAX)
                < credits_tabs.find("Credits").unwrap_or(0),
            "Limits stays before Credits:\n{credits_tabs}"
        );

        stopped.window.active_tab = LIMITS_TAB;
        let limits_card = paint_card(&mut stopped);
        assert!(
            limits_card.contains(NOT_DRAWN),
            "the Limits tab says included limits are not being drawn for the next request:\n{limits_card}"
        );
        assert!(
            limits_card.contains(NOT_READ),
            "the Limits tab says the included weekly allowance is not available yet:\n{limits_card}"
        );
        assert!(
            limits_card.contains("Console API credits: not available"),
            "the Limits tab keeps Console API credits: not available:\n{limits_card}"
        );
        assert!(
            limits_card.contains("Pacing for this week is not known yet."),
            "the week line stays when usage was not read:\n{limits_card}"
        );
        assert!(
            !limits_card.contains("Using limits") && !limits_card.contains("limits 0%"),
            "Using limits and limits 0% stay off while the card says included limits are not being drawn:\n{limits_card}"
        );
        assert!(
            !limits_card.contains("0% used") && !limits_card.contains("SuperGrok limits 0% used"),
            "an unread meter does not become a used percent:\n{limits_card}"
        );
    }

    /// When limits mode is on, the next model request draws included SuperGrok
    /// period limits, and an unread meter does not paint `limits 0%`.
    /// A live Team JWT with no live personal SuperGrok session must not paint
    /// `limits 1%` or `Using limits`.
    #[test]
    #[serial_test::serial]
    fn limits_mode_does_not_paint_limits_in_use_for_a_team_jwt_and_an_unread_meter_is_not_zero_percent()
     {
        use crate::actions::ActionRegistry;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::scrollback::render::ScratchBuffer;
        use xai_grok_shell::auth::limits_pins::{
            LimitsPins, MeterSource, next_request_draws_included_period_limits, save_limits_pins,
        };
        use xai_grok_shell::auth::{AuthMode, GrokAuth, upsert_supergrok_session};
        use xai_grok_test_support::EnvGuard;

        const NOT_DRAWN: &str = "Included limits are not being drawn for the next request.";

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        std::fs::write(
            home.path().join("config.toml"),
            "[auth]\npreferred_method = \"oidc\"\nauto_use_included_limits = true\n",
        )
        .expect("preferred oidc");
        save_limits_pins(&LimitsPins {
            stay_supergrok: true,
            use_console: false,
            meter_source: None,
            supergrok_identity: None,
        })
        .expect("stay pin");

        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-10-07T11:38:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let base = "https://auth.x.ai::limits-paint-fixture";
        let live = chrono::Utc::now() + chrono::Duration::days(1);
        let expired = chrono::Utc::now() - chrono::Duration::days(1);
        let session = |key: &str, user_id: &str, team: bool, expires_at| GrokAuth {
            key: key.into(),
            auth_mode: AuthMode::Oidc,
            user_id: user_id.into(),
            principal_type: if team {
                Some("Team".to_string())
            } else {
                Some("User".to_string())
            },
            principal_id: Some(user_id.to_string()),
            team_id: team.then(|| "team-fixture".to_string()),
            expires_at: Some(expires_at),
            ..GrokAuth::default()
        };
        let write_auth = |map: &std::collections::BTreeMap<String, GrokAuth>| {
            std::fs::write(
                home.path().join("auth.json"),
                serde_json::to_vec_pretty(map).expect("auth json"),
            )
            .expect("write auth");
        };

        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            base,
            session("tok-personal-expired", "u-personal-expired", false, expired),
        );
        map.get_mut(&format!("{base}::personal"))
            .expect("expired personal slot")
            .team_id = Some("stale-team".to_string());
        upsert_supergrok_session(
            &mut map,
            base,
            session("tok-team-only", "u-team", true, live),
        );
        write_auth(&map);
        assert!(
            !next_request_draws_included_period_limits(),
            "an expired personal session beside a live Team JWT does not draw included SuperGrok period limits"
        );

        save_limits_pins(&LimitsPins {
            meter_source: Some(MeterSource::Included),
            stay_supergrok: true,
            use_console: false,
            supergrok_identity: None,
        })
        .expect("included pin");
        assert!(
            next_request_draws_included_period_limits(),
            "an Included pin makes the next request draw included SuperGrok period limits even beside a Team JWT"
        );
        save_limits_pins(&LimitsPins {
            stay_supergrok: true,
            use_console: false,
            meter_source: None,
            supergrok_identity: None,
        })
        .expect("stay pin restored");

        let paint_header = |balance: &CreditBalance| -> String {
            crate::appearance::cache::set_hide_header(false);
            let mut agent = make_agent();
            agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
            agent.credit_balance = Some(balance.clone());
            let area = Rect::new(0, 0, 140, 40);
            let mut header = ratatui::buffer::Buffer::empty(area);
            let mut scratch = ScratchBuffer::new();
            agent.draw(
                area,
                &mut header,
                &ActionRegistry::defaults(),
                &mut scratch,
                None,
                false,
                BannerSlotParams::none(),
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
        };
        let one = weekly_bal(1.0, end);
        let team_header = paint_header(&one);
        assert!(
            !team_header.contains("limits 1%") && !team_header.contains("limits 0%"),
            "a Team JWT must not paint limits 1% or limits 0%:\n{team_header}"
        );
        let theme = Theme::default();
        let area = Rect::new(0, 0, 100, 40);
        let snap =
            LimitsSnapshot::from_billing(Some(&one), None, SamplingIdentityKind::SuperGrokSession);
        let mut state = LimitsModalState::new(snap);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        render_limits_modal(&mut buf, area, &mut state, &theme, false, now);
        let team_card = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            team_card.contains(NOT_DRAWN),
            "the card says included limits are not being drawn for the next request:\n{team_card}"
        );
        assert!(
            !team_card.contains("Using limits") && !team_card.contains("limits 0%"),
            "the footer must not say Using limits for a Team JWT:\n{team_card}"
        );
        assert!(
            !team_card.contains("1% used")
                && !team_card.contains("99%")
                && !team_card.contains("limits 1%"),
            "the card must not paint Included weekly allowance: 1% used or 99% remaining when included limits are not being drawn:\n{team_card}"
        );

        let mut live_map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut live_map,
            base,
            session("tok-team-only", "u-team", true, live),
        );
        upsert_supergrok_session(
            &mut live_map,
            base,
            session("tok-personal-included", "u-personal", false, live),
        );
        write_auth(&live_map);
        assert!(
            next_request_draws_included_period_limits(),
            "when limits mode is on and a live personal SuperGrok session exists, the next model request draws included SuperGrok period limits"
        );
        let known = weekly_bal(28.0, end);
        let personal_header = paint_header(&known);
        assert!(
            personal_header.contains("limits 28%"),
            "limits mode paints the short limits label when that meter is in use:\n{personal_header}"
        );
        assert!(
            !personal_header.contains("SuperGrok period"),
            "the header keeps the short limits label:\n{personal_header}"
        );
        let mut unread = weekly_bal(0.0, end);
        unread.included_usage_known = false;
        unread.usage_pct = 0.0;
        let unread_header = paint_header(&unread);
        assert!(
            !unread_header.contains("limits 0%"),
            "an unread meter does not paint limits 0%:\n{unread_header}"
        );
        let known_snap = LimitsSnapshot::from_billing(
            Some(&known),
            None,
            SamplingIdentityKind::SuperGrokSession,
        );
        let mut known_state = LimitsModalState::new(known_snap);
        let mut known_buf = ratatui::buffer::Buffer::empty(area);
        render_limits_modal(&mut known_buf, area, &mut known_state, &theme, false, now);
        let personal_card = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| known_buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            personal_card.contains("Using limits") && !personal_card.contains(NOT_DRAWN),
            "a live personal SuperGrok session says Using limits:\n{personal_card}"
        );
    }

    /// While plan mode is open, and the next request does not draw included
    /// SuperGrok period limits, the screen offers a Limits and Credits
    /// control. Opening it shows Limits before Credits. The control does not
    /// say `limits 0%` or `limits N%`.
    ///
    /// A live Team JWT leaves the in-use chip off. Header paint arms
    /// `hit_credits` only for that chip.
    #[test]
    #[serial_test::serial]
    fn plan_mode_screen_offers_limits_and_credits_when_included_limits_are_not_the_next_request() {
        use crate::actions::ActionRegistry;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::scrollback::render::ScratchBuffer;
        use xai_grok_shell::auth::limits_pins::{
            LimitsPins, MeterSource, next_request_draws_included_period_limits, save_limits_pins,
        };
        use xai_grok_shell::auth::{AuthMode, GrokAuth, upsert_supergrok_session};
        use xai_grok_test_support::EnvGuard;

        fn paints_limits_percent(text: &str) -> bool {
            for line in text.lines() {
                let mut rest = line;
                while let Some(idx) = rest.find("limits ") {
                    let after = &rest[idx + "limits ".len()..];
                    let digits = after.chars().take_while(|c| c.is_ascii_digit()).count();
                    if digits > 0 && after[digits..].starts_with('%') {
                        return true;
                    }
                    rest = &rest[idx + "limits ".len()..];
                }
            }
            false
        }

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        std::fs::write(
            home.path().join("config.toml"),
            "[auth]\npreferred_method = \"oidc\"\nauto_use_included_limits = true\n",
        )
        .expect("preferred oidc");
        save_limits_pins(&LimitsPins {
            stay_supergrok: true,
            use_console: false,
            meter_source: None,
            supergrok_identity: None,
        })
        .expect("stay pin");

        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-10-07T11:38:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let base = "https://auth.x.ai::plan-mode-limits-menu-fixture";
        let live = chrono::Utc::now() + chrono::Duration::days(1);
        let expired = chrono::Utc::now() - chrono::Duration::days(1);
        let session = |key: &str, user_id: &str, team: bool, expires_at| GrokAuth {
            key: key.into(),
            auth_mode: AuthMode::Oidc,
            user_id: user_id.into(),
            principal_type: if team {
                Some("Team".to_string())
            } else {
                Some("User".to_string())
            },
            principal_id: Some(user_id.to_string()),
            team_id: team.then(|| "team-fixture".to_string()),
            expires_at: Some(expires_at),
            ..GrokAuth::default()
        };
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            base,
            session("tok-personal-expired", "u-personal-expired", false, expired),
        );
        map.get_mut(&format!("{base}::personal"))
            .expect("expired personal slot")
            .team_id = Some("stale-team".to_string());
        upsert_supergrok_session(
            &mut map,
            base,
            session("tok-team-only", "u-team", true, live),
        );
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth json"),
        )
        .expect("write auth");
        assert!(
            !next_request_draws_included_period_limits(),
            "an expired personal session beside a live Team JWT does not draw included SuperGrok period limits"
        );
        save_limits_pins(&LimitsPins {
            meter_source: Some(MeterSource::Included),
            stay_supergrok: true,
            use_console: false,
            supergrok_identity: None,
        })
        .expect("included pin");
        assert!(
            next_request_draws_included_period_limits(),
            "an Included pin makes the next request draw included SuperGrok period limits even beside a Team JWT"
        );
        save_limits_pins(&LimitsPins {
            stay_supergrok: true,
            use_console: false,
            meter_source: None,
            supergrok_identity: None,
        })
        .expect("stay pin restored");

        crate::appearance::cache::set_hide_header(false);
        let mut agent = make_agent();
        agent.plan_mode_active = true;
        agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
        let balance = weekly_bal(1.0, end);
        agent.credit_balance = Some(balance.clone());
        let area = Rect::new(0, 0, 140, 40);
        let mut buf = Buffer::empty(area);
        let mut scratch = ScratchBuffer::new();
        agent.draw(
            area,
            &mut buf,
            &ActionRegistry::defaults(),
            &mut scratch,
            None,
            false,
            BannerSlotParams::none(),
            false,
            false,
            &mut Vec::new(),
            AppRenderParams::default(),
        );
        let screen = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            agent.plan_mode_active,
            "this draw is the plan-mode agent screen"
        );
        assert!(
            !next_request_draws_included_period_limits(),
            "the next request still does not draw included SuperGrok period limits"
        );
        assert!(
            !screen.contains("limits 0%"),
            "the control must not say limits 0% while the in-use chip is off:\n{screen}"
        );
        assert!(
            !paints_limits_percent(&screen),
            "the control must not say limits N% while included SuperGrok period limits are not the next request:\n{screen}"
        );

        let phrase = "Limits and Credits";
        let mut found = None;
        for (y, line) in screen.lines().enumerate() {
            if let Some(byte) = line.find(phrase) {
                let x = line[..byte].chars().count() as u16;
                found = Some((x, y as u16));
                break;
            }
        }
        let (x, y) = found.expect(
            "while plan mode is open and the next request does not draw included SuperGrok period limits, the screen must show a Limits and Credits control. The in-use chip is off, and header paint arms hit_credits only for that chip:\n{screen}",
        );
        let width = phrase.chars().count() as u16;
        for dx in 0..width {
            assert!(
                agent.hit_credits.contains(x + dx, y),
                "the Limits and Credits control must be the click that opens the card"
            );
        }
        let control_text: String = (0..width).map(|dx| buf[(x + dx, y)].symbol()).collect();
        assert_eq!(control_text, phrase);
        assert_ne!(control_text, "limits 0%");
        assert!(
            !paints_limits_percent(&control_text),
            "the control text must not be limits N%, got {control_text}"
        );

        let snap = LimitsSnapshot::from_billing(
            Some(&balance),
            None,
            SamplingIdentityKind::SuperGrokSession,
        );
        let mut state = LimitsModalState::new(snap);
        let theme = Theme::default();
        let modal_area = Rect::new(0, 0, 100, 40);
        let mut modal = Buffer::empty(modal_area);
        render_limits_modal(&mut modal, modal_area, &mut state, &theme, false, now);
        let card = (0..modal_area.height)
            .map(|row| {
                (0..modal_area.width)
                    .map(|col| modal[(col, row)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let tab_line = card
            .lines()
            .find(|line| line.contains("Limits") && line.contains("Credits"))
            .unwrap_or("");
        let limits_at = tab_line.find("Limits").expect("Limits tab");
        let credits_at = tab_line.find("Credits").expect("Credits tab");
        assert!(
            limits_at < credits_at,
            "opening the control shows Limits before Credits:\n{tab_line}"
        );
        assert_eq!(state.window.active_tab, LIMITS_TAB);
    }
}
