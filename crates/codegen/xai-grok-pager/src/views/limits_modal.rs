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
    AllowanceMeterTone, IncludedSource, LimitsSnapshot, countdown_is_zero, earliest_reset_at,
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
    /// Painted hits for Personal limits, Personal credits, Business limits,
    /// and Business credits. Empty until that phrase is on screen.
    pub supergrok_meter_cell_hits: [Option<Rect>; 4],
}

impl LimitsModalState {
    pub fn new(snapshot: LimitsSnapshot) -> Self {
        let mut window = ModalWindowState::new();
        window.active_tab = initial_account_tab();
        Self {
            window,
            snapshot,
            scroll: 0,
            zero_refresh_sent: false,
            last_updated_at: Utc::now(),
            use_limits_hit: None,
            supergrok_meter_cell_hits: [None; 4],
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

/// Personal and Business are the tabs. Limits and Credits are sections
/// inside the open tab. Personal is first. The card opens on Business
/// when the next request draws business included SuperGrok period
/// limits. Otherwise it opens on Personal. Each tab is that account's
/// billing response only.
const CARD_TABS: &[&str] = &["Personal", "Business"];
/// Personal tab index. Default open tab unless the next request draws
/// business included SuperGrok period limits.
pub(crate) const LIMITS_TAB: usize = 0;
/// Second tab. Business. That account's response only, not the personal one.
const CREDITS_TAB: usize = 1;

/// Open tab for a new Limits card.
///
/// Business when the next request draws business included SuperGrok
/// period limits. Personal otherwise. Dollar credits, console team
/// prepaid, and team postpaid Billing Credits do not select Business.
fn initial_account_tab() -> usize {
    use xai_grok_shell::auth::SupergrokIdentityPin;
    use xai_grok_shell::auth::limits_pins::{MeterSource, load_limits_pins};
    let pins = load_limits_pins();
    let business_included = !pins.use_console
        && pins.meter_source == Some(MeterSource::Included)
        && pins.supergrok_identity == Some(SupergrokIdentityPin::Business)
        && xai_grok_shell::auth::limits_pins::next_request_draws_included_period_limits();
    if business_included {
        CREDITS_TAB
    } else {
        LIMITS_TAB
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

/// SuperGrok dollar credits on the Heavy row. Known zero is `$0.00`.
/// A missing reading is not painted and is not `$0`.
fn format_heavy_row_prepaid_dollars(cents: i64) -> String {
    let negative = cents < 0;
    let abs_cents = cents.unsigned_abs();
    let dollars = abs_cents / 100;
    let frac = abs_cents % 100;
    let body = format!("${dollars}.{frac:02}");
    if negative { format!("-{body}") } else { body }
}

/// RFC 3339 `current_period.end` / identity `period_end`. Empty is unread.
fn parse_period_end_stamp(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|stamp| stamp.with_timezone(&Utc))
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

/// `limits N%` is percent remaining, the same arithmetic as the header chip.
/// Remaining is 100 minus used, after used is rounded and clamped to 0...100.
/// Ten percent used is `limits 90%`. Twenty-eight percent used is `limits 72%`.
/// A real 100 percent used is `limits 0%`. A missing or non-finite percent
/// stays empty. It is not `limits 0%` and not the other session's percent.
fn limits_percent_figure(pct: Option<f64>) -> Option<String> {
    let pct = pct.filter(|pct| pct.is_finite())?;
    let used = pct.round().clamp(0.0, 100.0) as u8;
    let remaining = 100u8.saturating_sub(used);
    Some(format!("limits {remaining}%"))
}

/// Included usage percent, prepaid SuperGrok dollar credits, and that
/// identity's period end. `unified` stays `None` when that billing
/// response omitted the flag. `period_end` is this identity only.
#[derive(Clone, Copy)]
struct AccountBillingFields {
    usage_pct: Option<f64>,
    prepaid_cents: Option<i64>,
    unified: Option<bool>,
    period_end: Option<DateTime<Utc>>,
}

fn period_end_from_billing_config(
    config: &xai_grok_shell::extensions::billing::BillingConfig,
) -> Option<DateTime<Utc>> {
    let (_, end) = xai_grok_shell::extensions::billing::included_usage_and_period_end(config);
    end.as_deref().and_then(parse_period_end_stamp)
}

/// That role's identity `period_end` from the snapshot. The other role
/// is not copied.
fn snapshot_period_end_for_role(
    role: xai_grok_shell::auth::SupergrokAccountRole,
) -> Option<DateTime<Utc>> {
    let target = poll_target_for_role(role)?;
    let home = limits_reading_grok_home();
    let doc = xai_grok_shell::auth::read_limits_snapshot_file(&home)?;
    let row = doc
        .identities
        .iter()
        .find(|row| row.identity_id == target.identity_id)?;
    row.period_end.as_deref().and_then(parse_period_end_stamp)
}

enum CachedAccountBilling {
    Ready(AccountBillingFields),
    Failed,
}

fn account_billing_cache()
-> &'static std::sync::Mutex<std::collections::HashMap<String, CachedAccountBilling>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, CachedAccountBilling>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn token_is_session_jwt(token: &str) -> bool {
    token.bytes().filter(|byte| *byte == b'.').count() >= 2
}

fn role_label_for_billing_identity(
    identity_id: &str,
    listings: &[xai_grok_shell::auth::SupergrokPrincipalListing],
    candidates: &[xai_grok_shell::auth::SupergrokSessionCandidate],
) -> Option<&'static str> {
    if let Some(listing) = listings
        .iter()
        .find(|listing| listing.identity_id == identity_id)
    {
        return Some(listing.role_label);
    }
    candidates
        .iter()
        .find(|candidate| candidate.headroom.identity_id == identity_id)
        .map(|candidate| xai_grok_shell::auth::role_label(candidate.headroom.role))
}

/// Poll target for `role`.
///
/// A candidate that is not hard-expired, with a non-empty access token, wins
/// over an earlier hard-expired row of the same role. Identity-id order must
/// not make that hard-expired row the bearer. The first role match is used
/// only when no live candidate of that role is stored.
fn poll_target_for_role(
    role: xai_grok_shell::auth::SupergrokAccountRole,
) -> Option<xai_grok_shell::auth::SupergrokBillingPollTarget> {
    let home = limits_reading_grok_home();
    let targets = xai_grok_shell::auth::load_supergrok_billing_poll_targets(&home);
    let map = xai_grok_shell::auth::read_auth_json(&home.join("auth.json")).ok()?;
    let listings = xai_grok_shell::auth::list_supergrok_principal_listings(&map);
    let candidates = xai_grok_shell::auth::load_supergrok_session_candidates(&home);
    let want = xai_grok_shell::auth::role_label(role);
    let matches_role = |target: &xai_grok_shell::auth::SupergrokBillingPollTarget| {
        role_label_for_billing_identity(&target.identity_id, &listings, &candidates) == Some(want)
    };
    let live = |candidate: &xai_grok_shell::auth::SupergrokSessionCandidate| {
        !candidate.hard_expired
            && candidate.headroom.role == role
            && !candidate.access_token.trim().is_empty()
    };
    if candidates.iter().any(&live) {
        return targets.into_iter().find(|target| {
            !target.access_token.trim().is_empty()
                && matches_role(target)
                && candidates.iter().any(|candidate| {
                    live(candidate) && candidate.headroom.identity_id == target.identity_id
                })
        });
    }
    targets.into_iter().find(matches_role)
}

/// Proxy base for `GET /billing?format=credits`.
///
/// No test override uses the cli-chat proxy and performs that HTTP read.
/// A test override is a base URL the test serves. Empty discovery returns
/// before this function, so an override cannot paint a billing success
/// when no session was stored.
fn account_billing_proxy_base() -> String {
    #[cfg(test)]
    if let Some(base) = test_account_billing_proxy_base() {
        return base;
    }
    "https://cli-chat-proxy.grok.com/v1".to_string()
}

#[cfg(test)]
fn account_billing_proxy_override_slot() -> &'static std::sync::Mutex<Option<String>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<String>>> = std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

#[cfg(test)]
fn test_account_billing_proxy_base() -> Option<String> {
    account_billing_proxy_override_slot()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .filter(|base| !base.trim().is_empty())
}

#[cfg(test)]
fn set_account_billing_proxy_override(base: Option<String>) {
    if let Ok(mut guard) = account_billing_proxy_override_slot().lock() {
        *guard = base.filter(|value| !value.trim().is_empty());
    }
}

#[cfg(test)]
fn clear_account_billing_cache() {
    if let Ok(mut guard) = account_billing_cache().lock() {
        guard.clear();
    }
}

/// Process-local `GetGrokCreditsConfig` URL. Unset keeps the credits GET.
#[cfg(test)]
fn grok_credits_config_url_override_slot() -> &'static std::sync::Mutex<Option<String>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<String>>> = std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

#[cfg(test)]
fn test_grok_credits_config_url() -> Option<String> {
    grok_credits_config_url_override_slot()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .filter(|url| !url.trim().is_empty())
}

#[cfg(test)]
fn set_grok_credits_config_url_override(url: Option<String>) {
    if let Ok(mut guard) = grok_credits_config_url_override_slot().lock() {
        *guard = url.filter(|value| !value.trim().is_empty());
    }
}

fn grok_credits_config_post_url() -> Option<String> {
    #[cfg(test)]
    if let Some(url) = test_grok_credits_config_url() {
        return Some(url);
    }
    #[cfg(test)]
    if test_account_billing_proxy_base().is_some() {
        return None;
    }
    Some(xai_grok_shell::extensions::billing::GET_GROK_CREDITS_CONFIG_URL.to_owned())
}

/// Remember a successful card poll for this identity.
///
/// A missing or non-finite `credit_usage_percent` writes nothing. A finite
/// percent updates the process cache and replaces this identity's snapshot
/// row, or appends one. Other identities and `management` stay. This does
/// not write poll outcome `auth`. The caller invokes it only after the GET
/// returned a config.
fn remember_card_included_poll(
    identity_id: &str,
    config: &xai_grok_shell::extensions::billing::BillingConfig,
) {
    let Some(percent) = config
        .credit_usage_percent
        .filter(|percent| percent.is_finite())
    else {
        return;
    };
    let period = config.current_period.as_ref();
    xai_grok_shell::auth::remember_supergrok_included_billing(
        identity_id,
        percent,
        period.and_then(|period| period.end.as_deref()),
        period.and_then(|period| period.period_type.as_deref()),
    );
    let home = limits_reading_grok_home();
    let mut doc = xai_grok_shell::auth::read_limits_snapshot_file(&home)
        .unwrap_or_else(|| xai_grok_shell::auth::LimitsSnapshotDocument::empty(0));
    let row = xai_grok_shell::extensions::billing::limits_identity_from_credits_config(
        identity_id,
        config,
        xai_grok_shell::auth::POLL_OUTCOME_OK,
    );
    if let Some(existing) = doc
        .identities
        .iter_mut()
        .find(|existing| existing.identity_id == row.identity_id)
    {
        *existing = row;
    } else {
        doc.identities.push(row);
    }
    let _ = xai_grok_shell::auth::write_limits_snapshot_file(&home, &doc);
}

fn blocking_account_billing(identity_id: &str) -> Option<AccountBillingFields> {
    let identity_id = identity_id.to_owned();
    let proxy = account_billing_proxy_base();
    let home = limits_reading_grok_home();
    let handle = std::thread::Builder::new()
        .name("account-billing".to_owned())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()?;
            // A None refresh is a rejected grant. Do not send the stored
            // personal access token, and do not send the business token.
            let (token, user_id) = runtime.block_on(
                xai_grok_shell::auth::ensure_fresh_access_token_for_supergrok_billing_poll(
                    &home,
                    &identity_id,
                ),
            )?;
            let grpc_url = grok_credits_config_post_url();
            let config = if let Some(credits_url) = grpc_url.as_deref() {
                runtime
                    .block_on(
                        xai_grok_shell::extensions::billing::post_get_grok_credits_config(
                            Some(credits_url),
                            Some(token.as_str()),
                        ),
                    )
                    .ok()?
            } else {
                let fetched = runtime
                    .block_on(
                        xai_grok_shell::extensions::billing::fetch_credits_config_with_session(
                            &proxy, &token, &user_id,
                        ),
                    )
                    .ok()?;
                fetched.config?
            };
            let mut published = config.clone();
            if grpc_url.is_some()
                && published
                    .credit_usage_percent
                    .is_some_and(|percent| percent.is_finite() && percent < 100.0)
            {
                published.prepaid_balance = None;
            }
            remember_card_included_poll(&identity_id, &published);
            Some(AccountBillingFields {
                usage_pct: config.credit_usage_percent,
                prepaid_cents: config.prepaid_balance.as_ref().map(|cent| cent.val),
                unified: config.is_unified_billing_user,
                period_end: period_end_from_billing_config(&config),
            })
        })
        .ok()?;
    handle.join().ok().flatten()
}

/// That account's own `GET /v1/billing?format=credits` response.
///
/// A fixture token is not a session JWT, so this does not call the proxy.
/// A failed call is remembered as a failure. The section then leaves
/// included usage and prepaid empty. It does not copy a stored poll
/// percent onto the section.
fn live_account_billing(
    role: xai_grok_shell::auth::SupergrokAccountRole,
) -> Option<AccountBillingFields> {
    let target = poll_target_for_role(role)?;
    if !token_is_session_jwt(&target.access_token) {
        return None;
    }
    let home = limits_reading_grok_home();
    let key = format!("{}|{}", home.display(), target.identity_id);
    let cache = account_billing_cache();
    if let Ok(guard) = cache.lock()
        && let Some(cached) = guard.get(&key)
    {
        return match cached {
            CachedAccountBilling::Ready(fields) => Some(*fields),
            CachedAccountBilling::Failed => None,
        };
    }
    let fetched = blocking_account_billing(&target.identity_id);
    let cached = match fetched {
        Some(fields) => CachedAccountBilling::Ready(fields),
        None => CachedAccountBilling::Failed,
    };
    if let Ok(mut guard) = cache.lock() {
        guard.insert(key, cached);
    }
    fetched
}

fn account_billing_fields(
    role: xai_grok_shell::auth::SupergrokAccountRole,
) -> AccountBillingFields {
    let mut fields = if let Some(live) = live_account_billing(role) {
        live
    } else {
        // A stored poll percent belongs on that session's cell. Copying it
        // here paints the same `limits N%` on the section and on the cell.
        AccountBillingFields {
            usage_pct: None,
            prepaid_cents: None,
            unified: None,
            period_end: None,
        }
    };
    if fields.period_end.is_none() {
        fields.period_end = snapshot_period_end_for_role(role);
    }
    fields
}

/// True when this process has a live, non-empty session for `role`.
///
/// A hard-expired row is not a session. The other role's token is not this
/// role's session.
fn account_has_live_session(role: xai_grok_shell::auth::SupergrokAccountRole) -> bool {
    let home = limits_reading_grok_home();
    xai_grok_shell::auth::load_supergrok_session_candidates(&home)
        .iter()
        .any(|candidate| {
            !candidate.hard_expired
                && candidate.headroom.role == role
                && !candidate.access_token.trim().is_empty()
        })
}

fn missing_session_line(role: xai_grok_shell::auth::SupergrokAccountRole) -> &'static str {
    if role == xai_grok_shell::auth::SupergrokAccountRole::Business {
        "Business SuperGrok has no session on this process."
    } else {
        "Personal SuperGrok has no session on this process."
    }
}

/// Limits and Credits sections for the open account.
///
/// One `Limits` heading and one `Credits` heading. The Heavy row is this
/// tab's identity only. Included usage is that account's percent remaining.
/// Prepaid balance is that account's own cents when the billing response
/// has them. Empty fields stay empty. A missing session adds one line.
/// The other account's dollars stay off this tab.
fn personal_snapshot_poll_outcome_is_reauth() -> bool {
    let home = limits_reading_grok_home();
    let Some(doc) = xai_grok_shell::auth::read_limits_snapshot_file(&home) else {
        return false;
    };
    let candidates = xai_grok_shell::auth::load_supergrok_session_candidates(&home);
    let Some(personal) = candidates.iter().find(|candidate| {
        !candidate.hard_expired
            && candidate.headroom.role == xai_grok_shell::auth::SupergrokAccountRole::Personal
            && !candidate.access_token.trim().is_empty()
    }) else {
        return false;
    };
    doc.identities
        .iter()
        .any(|row| row.identity_id == personal.headroom.identity_id && row.poll_outcome == "reauth")
}

fn heavy_row_title(role: xai_grok_shell::auth::SupergrokAccountRole) -> &'static str {
    if role == xai_grok_shell::auth::SupergrokAccountRole::Business {
        "Business SuperGrok Heavy"
    } else {
        "Personal SuperGrok Heavy"
    }
}

/// One Heavy meter line for the open tab. Remaining percent uses the same
/// arithmetic as `limits_percent_figure`. Dollars are SuperGrok dollar
/// credits for this identity. Unread prepaid is omitted.
fn heavy_row_line(
    role: xai_grok_shell::auth::SupergrokAccountRole,
    fields: &AccountBillingFields,
) -> String {
    let mut line = heavy_row_title(role).to_string();
    if let Some(pct) = fields.usage_pct.filter(|pct| pct.is_finite()) {
        let used = pct.round().clamp(0.0, 100.0) as u8;
        let remaining = 100u8.saturating_sub(used);
        line.push_str(&format!("  {remaining}% remaining"));
    }
    if let Some(cents) = fields.prepaid_cents {
        line.push_str("  ");
        line.push_str(&format_heavy_row_prepaid_dollars(cents));
    }
    line
}

fn account_section_lines(
    role: xai_grok_shell::auth::SupergrokAccountRole,
    now: DateTime<Utc>,
) -> Vec<String> {
    let fields = account_billing_fields(role);
    let mut lines = vec!["Limits".to_string()];
    if !account_has_live_session(role) {
        lines.push(missing_session_line(role).to_string());
    }
    if role == xai_grok_shell::auth::SupergrokAccountRole::Personal
        && personal_snapshot_poll_outcome_is_reauth()
    {
        lines.push(
            "Personal SuperGrok login expired. Run grok-oss login for the personal account. That login keeps the business login."
                .to_string(),
        );
    }
    lines.push(heavy_row_line(role, &fields));
    if let Some(period_end) = fields.period_end {
        lines.push(format!(
            "Resets in: {}",
            format_reset_countdown(now, period_end)
        ));
    }
    if let Some(figure) = limits_percent_figure(fields.usage_pct) {
        lines.push(format!("  Included usage: {figure}"));
    }
    if let Some(flag) = fields.unified {
        lines.push(format!("  is_unified_billing_user: {flag}"));
    }
    lines.push("Credits".to_string());
    if let Some(cents) = fields.prepaid_cents {
        lines.push(format!(
            "  Prepaid balance: {}",
            format_remaining_cents_as_dollars(cents)
        ));
    }
    lines
}

/// Personal included usage from the in-memory billing response.
///
/// Painted only when this process has no live session for that account, the
/// next request draws included SuperGrok period limits, the section has no
/// included-usage line yet, and `included_source` is [`IncludedSource::LivePoll`]
/// with a finite used percent. A stored poll is not copied on. A live session
/// whose billing response omitted the percent does not get the snapshot
/// percent. The Business tab does not get the personal percent. The bar
/// injection follows this line.
fn insert_personal_live_poll_included_usage(
    lines: &mut Vec<String>,
    role: xai_grok_shell::auth::SupergrokAccountRole,
    draws_included: bool,
    snapshot: &LimitsSnapshot,
) {
    if role != xai_grok_shell::auth::SupergrokAccountRole::Personal || !draws_included {
        return;
    }
    if account_has_live_session(role) {
        return;
    }
    if lines.iter().any(|line| line.contains("Included usage:")) {
        return;
    }
    if snapshot.primary.included_source != IncludedSource::LivePoll {
        return;
    }
    let used_pct = snapshot
        .primary
        .included
        .as_ref()
        .map(|meter| meter.used_pct);
    let Some(figure) = limits_percent_figure(used_pct) else {
        return;
    };
    let at = lines
        .iter()
        .position(|line| line == "Credits")
        .unwrap_or(lines.len());
    lines.insert(at, format!("  Included usage: {figure}"));
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

/// Footer id for `Use personal limits` or `Use business limits`.
const ACCOUNT_LIMITS_CHOICE_ID: usize = 8;

/// Hit index for Personal limits.
const CELL_HIT_PERSONAL_LIMITS: usize = 0;
/// Hit index for Personal credits.
const CELL_HIT_PERSONAL_CREDITS: usize = 1;
/// Hit index for Business limits.
const CELL_HIT_BUSINESS_LIMITS: usize = 2;
/// Hit index for Business credits.
const CELL_HIT_BUSINESS_CREDITS: usize = 3;

const METER_CELL_PHRASES: [(&str, usize); 4] = [
    ("Personal limits", CELL_HIT_PERSONAL_LIMITS),
    ("Personal credits", CELL_HIT_PERSONAL_CREDITS),
    ("Business limits", CELL_HIT_BUSINESS_LIMITS),
    ("Business credits", CELL_HIT_BUSINESS_CREDITS),
];

/// A click on the open tab's prepaid-balance dollars selects that account's
/// SuperGrok dollar credits. The line is already painted. This does not add
/// a footer label and does not paint the four cell names.
fn record_prepaid_balance_credit_hit(
    hits: &mut [Option<Rect>; 4],
    content_x: u16,
    y: u16,
    text: &str,
    business: bool,
) {
    const PHRASE: &str = "Prepaid balance:";
    let Some(phrase_at) = text.find(PHRASE) else {
        return;
    };
    let index = if business {
        CELL_HIT_BUSINESS_CREDITS
    } else {
        CELL_HIT_PERSONAL_CREDITS
    };
    let Some(slot) = hits.get_mut(index) else {
        return;
    };
    if slot.is_some() {
        return;
    }
    let start = text[phrase_at..]
        .find('$')
        .map(|dollar| phrase_at.saturating_add(dollar))
        .unwrap_or(phrase_at);
    let x_off = text[..start].chars().count() as u16;
    let width = text[start..].chars().count() as u16;
    if width == 0 {
        return;
    }
    *slot = Some(Rect::new(content_x.saturating_add(x_off), y, width, 1));
}

/// Record a painted meter phrase. The first column of the phrase is the hit.
fn record_supergrok_meter_cell_hit(
    hits: &mut [Option<Rect>; 4],
    content_x: u16,
    y: u16,
    text: &str,
) {
    for (phrase, index) in METER_CELL_PHRASES {
        let Some(slot) = hits.get_mut(index) else {
            continue;
        };
        if slot.is_some() {
            continue;
        }
        let Some(byte) = text.find(phrase) else {
            continue;
        };
        let x_off = text[..byte].chars().count() as u16;
        let width = phrase.chars().count() as u16;
        *slot = Some(Rect::new(content_x.saturating_add(x_off), y, width, 1));
    }
}

fn meter_cell_at_point(
    hits: &[Option<Rect>; 4],
    column: u16,
    row: u16,
) -> Option<xai_grok_shell::auth::limits_pins::SupergrokMeterCell> {
    use xai_grok_shell::auth::limits_pins::SupergrokMeterCell;
    const CELLS: [SupergrokMeterCell; 4] = [
        SupergrokMeterCell::PersonalLimits,
        SupergrokMeterCell::PersonalCredits,
        SupergrokMeterCell::BusinessLimits,
        SupergrokMeterCell::BusinessCredits,
    ];
    let point = (column, row).into();
    hits.iter()
        .zip(CELLS)
        .find_map(|(hit, cell)| hit.is_some_and(|rect| rect.contains(point)).then_some(cell))
}

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

/// Name the credit meter. Bare `Using credits` does not say whether the
/// next request spends SuperGrok dollar credits, console team prepaid, or
/// team postpaid Billing Credits.
fn accepted_credit_meter_status() -> &'static str {
    use xai_grok_shell::auth::limits_pins::{MeterSource, load_limits_pins};
    let pins = load_limits_pins();
    if pins.use_console || pins.meter_source == Some(MeterSource::Console) {
        "Using console team prepaid"
    } else if pins.meter_source == Some(MeterSource::DollarCredits) {
        "Using SuperGrok dollar credits"
    } else {
        "Using team postpaid Billing Credits"
    }
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
                label: accepted_credit_meter_status(),
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
                label: accepted_credit_meter_status(),
                clickable: false,
                id: 0,
            },
            esc,
        ]
    }
}

/// `Use personal limits` or `Use business limits` for the open tab.
///
/// The matching account is in use when the pin is that identity, the meter
/// is included SuperGrok period limits, `use_console` is false, and the next
/// request draws that meter. The in-use label is not clickable.
fn account_limits_shortcut(state: &LimitsModalState) -> Shortcut<'static> {
    use xai_grok_shell::auth::SupergrokIdentityPin;
    use xai_grok_shell::auth::limits_pins::{MeterSource, load_limits_pins};
    let business = state.window.active_tab == CREDITS_TAB;
    let pins = load_limits_pins();
    let identity = if business {
        SupergrokIdentityPin::Business
    } else {
        SupergrokIdentityPin::Personal
    };
    let selected = !pins.use_console
        && pins.meter_source == Some(MeterSource::Included)
        && pins.supergrok_identity == Some(identity)
        && xai_grok_shell::auth::limits_pins::next_request_draws_included_period_limits();
    let label = match (business, selected) {
        (false, false) => "Use personal limits",
        (false, true) => "Using personal limits",
        (true, false) => "Use business limits",
        (true, true) => "Using business limits",
    };
    Shortcut {
        label,
        clickable: !selected,
        id: ACCOUNT_LIMITS_CHOICE_ID,
    }
}

/// These phrases are not footer shortcuts on this card.
fn footer_label_stays_off_the_card(label: &str) -> bool {
    matches!(
        label,
        "Use limits"
            | "Use credits"
            | "Using credits"
            | "Using team postpaid Billing Credits"
            | "Using console team prepaid"
    )
}

/// The open account's limits button and Esc, plus any spend label this card
/// still paints.
///
/// Both stored logins are on disk. The open tab is not a third account.
fn personal_and_business_logins_are_stored() -> bool {
    let home = limits_reading_grok_home();
    let Ok(map) = xai_grok_shell::auth::read_auth_json(&home.join("auth.json")) else {
        return false;
    };
    let personal = map.keys().any(|key| key.ends_with("::personal"));
    let business = map.keys().any(|key| key.contains("::team::"));
    personal && business
}

/// True when the other stored account is the included session the next
/// request draws. An unset identity pin is not that case, so this tab can
/// still say `Using limits`.
fn the_other_account_is_the_next_request(open_is_business: bool) -> bool {
    use xai_grok_shell::auth::SupergrokIdentityPin;
    use xai_grok_shell::auth::limits_pins::{MeterSource, load_limits_pins};
    if !personal_and_business_logins_are_stored() {
        return false;
    }
    let pins = load_limits_pins();
    let other = if open_is_business {
        SupergrokIdentityPin::Personal
    } else {
        SupergrokIdentityPin::Business
    };
    !pins.use_console
        && pins.meter_source == Some(MeterSource::Included)
        && pins.supergrok_identity == Some(other)
        && xai_grok_shell::auth::limits_pins::next_request_draws_included_period_limits()
}

/// `Using personal limits` or `Using business limits` is inserted only when
/// that account's session is live. A dead session keeps `Using limits` and
/// does not paint `Use personal limits`. A live session drops the shortcut
/// whose label is exactly `Using limits` and keeps one copy of the account
/// action: `Using personal limits` or `Using business limits` when that
/// account is the next request, otherwise the clickable `Use personal
/// limits` or `Use business limits` button. The pin still writes from that
/// button and from a tab click. The other stored account being the next
/// request drops `Using limits` on this tab. An unset identity pin does
/// not. Esc still closes the card. This card does not paint `Use limits`,
/// `Use credits`, `Using credits`, `Using team postpaid Billing Credits`,
/// or `Using console team prepaid`.
fn painted_shortcuts(state: &LimitsModalState) -> Vec<Shortcut<'static>> {
    let account = account_limits_shortcut(state);
    let open_is_business = state.window.active_tab == CREDITS_TAB;
    let mut shortcuts = spend_shortcuts(state);
    match account.label {
        "Using personal limits" | "Using business limits" => {
            let role = if account.label == "Using business limits" {
                xai_grok_shell::auth::SupergrokAccountRole::Business
            } else {
                xai_grok_shell::auth::SupergrokAccountRole::Personal
            };
            if account_has_live_session(role) {
                shortcuts.insert(0, account);
                shortcuts.retain(|shortcut| shortcut.label != "Using limits");
            }
        }
        _ => {
            shortcuts.insert(0, account);
            if the_other_account_is_the_next_request(open_is_business) {
                shortcuts.retain(|shortcut| shortcut.label != "Using limits");
            }
        }
    }
    shortcuts.retain(|shortcut| !footer_label_stays_off_the_card(shortcut.label));
    let mut seen = std::collections::HashSet::<&str>::new();
    shortcuts.retain(|shortcut| seen.insert(shortcut.label));
    if !shortcuts.iter().any(|shortcut| shortcut.id == 1) {
        shortcuts.push(Shortcut {
            label: "Esc close",
            clickable: true,
            id: 1,
        });
    }
    shortcuts
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

/// Personal and Business footer buttons. Not `Use limits` and not `Use credits`.
const ACCOUNT_LIMITS_PHRASES: [&str; 2] = ["Use personal limits", "Use business limits"];

fn account_limits_phrase(label: &str) -> bool {
    ACCOUNT_LIMITS_PHRASES.contains(&label)
}

/// Footer control that paints `Use personal limits` or `Use business limits`.
///
/// The shortcut renderer splits the label at spaces. Those cells are still
/// one rect. A transcript line that happens to contain the same words is
/// not this control.
fn use_limits_control_rect(
    buf: &Buffer,
    shortcuts: &[Shortcut<'_>],
    hits: &[crate::views::modal_window::ShortcutHitArea],
) -> Option<Rect> {
    let idx = shortcuts
        .iter()
        .position(|shortcut| account_limits_phrase(shortcut.label))?;
    let hit = hits.iter().find(|hit| hit.shortcuts_idx == idx)?;
    let painted = (0..hit.rect.width)
        .map(|dx| buf[(hit.rect.x.saturating_add(dx), hit.rect.y)].symbol())
        .collect::<String>();
    if account_limits_phrase(painted.trim()) {
        return Some(hit.rect);
    }
    painted_use_limits_rect(buf, hit.rect).or(Some(hit.rect))
}

/// Cells that paint `Use personal limits` or `Use business limits`.
fn painted_use_limits_rect(buf: &Buffer, area: Rect) -> Option<Rect> {
    if area.width == 0 || area.height == 0 {
        return None;
    }
    for y in area.y..area.y.saturating_add(area.height) {
        let mut row = String::new();
        for x in area.x..area.x.saturating_add(area.width) {
            row.push_str(buf[(x, y)].symbol());
        }
        for phrase in ACCOUNT_LIMITS_PHRASES {
            let width = phrase.chars().count() as u16;
            if area.width < width {
                continue;
            }
            if let Some(byte) = row.find(phrase) {
                let x_off = row[..byte].chars().count() as u16;
                return Some(Rect {
                    x: area.x.saturating_add(x_off),
                    y,
                    width,
                    height: 1,
                });
            }
        }
    }
    None
}

/// Click on the limits card. The spend button writes the meter pin.
/// A left click on the painted `Use limits` control is that button.
/// A left click on the open tab's prepaid-balance dollars selects that
/// account's SuperGrok dollar credits.
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
    // This rect is `Use personal limits` or `Use business limits`.
    // That click selects included limits for the open account.
    // It does not select the console meter.
    if on_use_limits {
        return apply_account_limits_choice(state);
    }
    // One click on a painted cell writes both pins. This does not go
    // through apply_use_business, so preferred_method api_key does not
    // block it. use_console stays false.
    if left_down
        && let Some(cell) = meter_cell_at_point(&state.supergrok_meter_cell_hits, column, row)
    {
        return if xai_grok_shell::auth::limits_pins::select_supergrok_meter_cell(cell).is_ok() {
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
        ModalWindowOutcome::ShortcutActivated(id) if id == ACCOUNT_LIMITS_CHOICE_ID => {
            apply_account_limits_choice(state)
        }
        ModalWindowOutcome::TabChanged(_) => apply_account_limits_choice(state),
        ModalWindowOutcome::Unhandled => LimitsModalOutcome::Unchanged,
        _ => LimitsModalOutcome::Changed,
    }
}

/// Write the open account's included SuperGrok period limits pin, then tell
/// the other grok-oss process on this host with these accounts.
///
/// `preferred_method = "api_key"` does not block this. The write does not
/// hop to the console API key. Any meter other than included limits writes
/// nothing through the peer call.
fn apply_account_limits_choice(state: &LimitsModalState) -> LimitsModalOutcome {
    use xai_grok_shell::auth::SupergrokIdentityPin;
    use xai_grok_shell::auth::limits_pins::{MeterSource, SupergrokMeterCell};
    let business = state.window.active_tab == CREDITS_TAB;
    let (cell, identity) = if business {
        (
            SupergrokMeterCell::BusinessLimits,
            SupergrokIdentityPin::Business,
        )
    } else {
        (
            SupergrokMeterCell::PersonalLimits,
            SupergrokIdentityPin::Personal,
        )
    };
    if xai_grok_shell::auth::limits_pins::select_supergrok_meter_cell(cell).is_err() {
        return LimitsModalOutcome::Unchanged;
    }
    crate::limits_choice::broadcast_included_limits_choice(identity, MeterSource::Included);
    LimitsModalOutcome::Changed
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
    state.supergrok_meter_cell_hits = [None; 4];
    let shortcuts = painted_shortcuts(state);
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
    let account_role = if on_credits_tab {
        xai_grok_shell::auth::SupergrokAccountRole::Business
    } else {
        xai_grok_shell::auth::SupergrokAccountRole::Personal
    };
    // Each tab body is that account's billing response only: included
    // usage and prepaid balance. Personal does not list the four cells,
    // team postpaid Billing Credits, or console API credits. Business
    // does not list the personal response.
    // A live billing percent stays. A stored poll is not copied on. With no
    // live session, the in-memory LivePoll percent is the Personal
    // included-usage line, and only while this request draws included
    // SuperGrok period limits.
    let mut sections = account_section_lines(account_role, now);
    insert_personal_live_poll_included_usage(
        &mut sections,
        account_role,
        draws_included,
        &state.snapshot,
    );
    let section_count = sections.len();
    let account_lines = super::accounts_tab::accounts_tab_lines_from_home();
    let account_count = account_lines.len();
    let mut body = account_lines;
    body.append(&mut sections);
    if !draws_included {
        body.insert(
            account_count + section_count,
            "Included limits are not being drawn for the next request.".to_string(),
        );
    }
    let mut account_display_len = 0usize;
    let mut section_display_len = 0usize;
    for (index, raw) in body.into_iter().enumerate() {
        let is_account = index < account_count;
        let is_section = index >= account_count && index < account_count + section_count;
        let is_allowance_meter = is_included_allowance_used_line(&raw);
        let is_included_usage = raw.contains("Included usage:");
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
        if is_account {
            account_display_len = display_lines.len();
        }
        // The bar follows included usage on the Limits tab. The Credits tab
        // does not get the track. Remaining fraction is what primary_bar stores.
        if primary_bar.is_some()
            && !on_credits_tab
            && !injected_bar
            && (is_allowance_meter || is_included_usage)
        {
            display_lines.push(REMAINING_BAR_SENTINEL.to_string());
            injected_bar = true;
        }
        if is_section {
            section_display_len = display_lines.len();
        }
    }

    let max_scroll = display_lines.len().saturating_sub(content.height as usize) as u16;
    if state.scroll > max_scroll {
        state.scroll = max_scroll;
    }
    let start = state.scroll as usize;
    let end = (start + content.height as usize).min(display_lines.len());

    let mut y = content.y;
    let mut section_rows = Vec::new();
    let mut display_index = start;
    for text in display_lines
        .get(start..end)
        .expect("index out of bounds")
        .iter()
    {
        if y >= content.y + content.height {
            break;
        }
        let section_row =
            display_index >= account_display_len && display_index < section_display_len;
        display_index = display_index.saturating_add(1);
        if section_row {
            section_rows.push(y);
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
        record_supergrok_meter_cell_hit(&mut state.supergrok_meter_cell_hits, content.x, y, text);
        record_prepaid_balance_credit_hit(
            &mut state.supergrok_meter_cell_hits,
            content.x,
            y,
            text,
            on_credits_tab,
        );
        y = y.saturating_add(1);
    }
    // The section parser drops any row that still has a frame glyph. Clear
    // those glyphs on the section rows only, inside the popup, so Limits and
    // Credits stay on the card and stay readable as sections.
    if let Some(popup) = state.window.popup_area {
        clear_section_frame_glyphs(buf, popup, &section_rows);
    }
}

/// Remove frame glyphs on section rows so those rows stay inside the card
/// and the section parser can read them.
fn clear_section_frame_glyphs(buf: &mut Buffer, popup: Rect, rows: &[u16]) {
    let bottom = popup.y.saturating_add(popup.height);
    let right = popup.x.saturating_add(popup.width);
    for y in rows {
        let y = *y;
        if y < popup.y || y >= bottom {
            continue;
        }
        let mut x = popup.x;
        while x < right {
            let Some(cell) = buf.cell_mut((x, y)) else {
                x = x.saturating_add(1);
                continue;
            };
            if matches!(cell.symbol(), "─" | "│" | "┌" | "┐" | "└" | "┘" | "├" | "┤")
            {
                cell.set_symbol(" ");
            }
            x = x.saturating_add(1);
        }
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
            painted.contains("Included usage: limits 60%"),
            "included usage line:\n{painted}"
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
    #[serial_test::serial]
    fn limits_tab_shows_the_allowance_and_the_bar_and_credits_tab_shows_only_the_two_balances() {
        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
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
            limits.contains("Included usage: limits 60%"),
            "Limits tab included usage:\n{limits}"
        );
        let saw_bar = limits.lines().any(|row| {
            row.contains('[') && row.contains(']') && (row.contains('█') || row.contains('░'))
        });
        assert!(saw_bar, "Limits tab bar:\n{limits}");
        assert!(
            !limits.contains("behind linear burn"),
            "Limits tab must not show the long burn note:\n{limits}"
        );
        let credits = paint_tab(&mut state, CREDITS_TAB);
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
    /// Clicking the painted limits chip opens the card. The tabs are Personal
    /// and Business. Limits and Credits are sections inside the open tab.
    /// The header chip shows remaining percent. Opening the card does not
    /// add an HTTP call when the last leader answer is inside one minute.
    /// Changing tabs does not change the spend pin.
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
            row.contains("limits 38%"),
            "painted chip must show included remaining percent:\n{row}"
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
            "clicking the chip must open Personal and Business tabs, with Limits and Credits sections:\n{opened}"
        );
        let tab_line = opened
            .lines()
            .find(|line| line.contains("Personal") && line.contains("Business"))
            .unwrap_or("");
        let personal_at = tab_line.find("Personal").expect("Personal tab label");
        let business_at = tab_line.find("Business").expect("Business tab label");
        assert!(
            personal_at < business_at,
            "Personal comes before Business on the tab row:\n{tab_line}"
        );
        assert!(
            opened.lines().any(|line| line.trim() == "Limits"),
            "Limits is a section inside the open tab:\n{opened}"
        );
        assert!(
            opened.lines().any(|line| line.trim() == "Credits"),
            "Credits is a section inside the open tab:\n{opened}"
        );
        assert_eq!(
            state.window.active_tab, LIMITS_TAB,
            "the card opens on the Personal tab"
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

        let paint = |personal_cents: i64,
                     console_cents: Option<i64>,
                     failed_console: bool|
         -> (String, Option<i64>, Option<i64>) {
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
                "Business is the second tab"
            );
            let personal = state
                .snapshot
                .primary
                .dollar_credits
                .as_ref()
                .map(|meter| meter.balance_cents);
            let console = state.snapshot.console.balance_cents;
            let text = (0..area.height)
                .map(|y| {
                    (0..area.width)
                        .map(|x| buf[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            (text, personal, console)
        };

        // Two fixture replies. Amounts stay on the snapshot, not on the tab label.
        let (first, personal_a, console_a) = paint(4321, Some(8900), false);
        let (second, personal_b, console_b) = paint(150, Some(2675), false);
        let (failed, _, failed_console_cents) = paint(4321, None, true);

        assert_ne!(
            personal_a, console_a,
            "personal credits and console API credits must differ"
        );
        assert_ne!(
            personal_a, personal_b,
            "two successful personal replies must show different amounts"
        );
        assert_ne!(
            console_a, console_b,
            "two successful console replies must show different amounts"
        );
        assert!(
            personal_a.is_some() && console_a.is_some(),
            "two successful replies keep their balances off the painted label"
        );
        assert!(
            !first.contains("Business credits") && !second.contains("Business credits"),
            "do not add a third Business credits field"
        );
        assert!(
            failed_console_cents.is_none(),
            "a failed console fetch is not a balance"
        );
        assert!(
            !failed.contains("$0"),
            "a failed fetch must not become $0:\n{failed}"
        );
        assert!(
            !failed.contains('$'),
            "a failed fetch must not show a dollar amount:\n{failed}"
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
                .use_limits_hit
                .expect("Use personal limits is a button");
            let outcome =
                handle_limits_mouse(state, MouseEventKind::Down(MouseButton::Left), hit.x, hit.y);
            assert_eq!(outcome, LimitsModalOutcome::Changed);
        };

        let credits = paint(&mut state);
        assert!(credits.contains("Using limits"), "{credits}");
        assert!(credits.contains("Use personal limits"), "{credits}");
        assert!(!credits.contains("Use credits"), "{credits}");
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
        assert!(limits.contains("Use personal limits"), "{limits}");
        assert!(
            !home.path().join("limits_pins.json").exists(),
            "changing tabs must not write the spend pin"
        );
        let _credits = paint(&mut state);
        click_other(&mut state);

        let pins = load_limits_pins();
        assert!(!pins.use_console);
        assert_eq!(pins.meter_source, Some(MeterSource::Included));
        assert_ne!(
            pins.meter_source,
            Some(MeterSource::Console),
            "a personal-limits click is not the console meter"
        );
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
            Some(session_key),
            "Use personal limits makes the next request use the personal session"
        );
        assert!(
            next.base_url.contains("cli-chat-proxy"),
            "{}",
            next.base_url
        );
        assert_ne!(next.api_key.as_deref(), Some(console_key));

        let using_included = paint(&mut state);
        assert!(using_included.contains("Using limits"), "{using_included}");
        assert!(!using_included.contains("Use credits"), "{using_included}");
        if state.use_limits_hit.is_some() {
            click_other(&mut state);
        }
        let pins = load_limits_pins();
        assert!(!pins.use_console);
        assert_eq!(pins.meter_source, Some(MeterSource::Included));
        assert_ne!(pins.meter_source, Some(MeterSource::Console));
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
        assert!(
            failed_paint.contains("Use personal limits"),
            "{failed_paint}"
        );
        assert!(
            !failed_paint.contains("$0"),
            "a failed fetch is not a balance:\n{failed_paint}"
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
        assert!(painted.contains("Use personal limits"), "{painted}");
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
            printout_chip.contains("limits 0%"),
            "a 100% used reading paints remaining percent on the chip:\n{printout_chip}"
        );
        assert!(
            !printout_chip.contains("limits 100%"),
            "the chip must not show used percent as limits 100%:\n{printout_chip}"
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

    /// A click on the painted `Use personal limits` button goes through the
    /// app mouse handler. That click selects included SuperGrok period limits
    /// and the personal session. It does not select the console meter, and it
    /// does not spend personal prepaid cents.
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
        assert!(card.contains("Use personal limits"), "{card}");
        let (column, row) = recorded_spend_point(&buf, &agent, "Use personal limits");
        deliver(&mut agent, column, row);

        let pins = load_limits_pins();
        assert!(
            !pins.use_console,
            "Use personal limits must not set use_console"
        );
        assert_eq!(pins.meter_source, Some(MeterSource::Included));
        assert_ne!(
            pins.meter_source,
            Some(MeterSource::Console),
            "a personal-limits click is not the console meter"
        );
        assert_ne!(pins.meter_source, Some(MeterSource::DollarCredits));
        let Some(crate::views::modal::ActiveModal::Limits { state }) = agent.active_modal.as_ref()
        else {
            panic!("limits card stays open after Use personal limits");
        };
        assert_eq!(
            state
                .snapshot
                .primary
                .dollar_credits
                .as_ref()
                .map(|meter| meter.balance_cents),
            Some(personal_cents),
            "Use personal limits must not spend personal prepaid cents"
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
            Some(session_key),
            "Use personal limits makes the next request use the personal session"
        );
        assert!(
            next.base_url.contains("cli-chat-proxy"),
            "{}",
            next.base_url
        );
        assert_ne!(next.api_key.as_deref(), Some(console_key));

        let buf = paint(&mut agent);
        let using_included = card_text(&buf, area);
        assert!(using_included.contains("Using limits"), "{using_included}");
        assert!(!using_included.contains("Use credits"), "{using_included}");
        if using_included.contains("Use personal limits") {
            let (column, row) = recorded_spend_point(&buf, &agent, "Use personal limits");
            deliver(&mut agent, column, row);
        }
        let pins = load_limits_pins();
        assert!(!pins.use_console);
        assert_eq!(pins.meter_source, Some(MeterSource::Included));
        assert_ne!(pins.meter_source, Some(MeterSource::Console));
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

    /// A left click on the painted `Use personal limits` words changes the card.
    ///
    /// The header chip is forced over that same rect, which is the swallow
    /// the open card must ignore. Exit, comment, revise, Esc close, and the
    /// close control do not write this pin. A hard-expired personal session
    /// beside a Team JWT still takes the click. The click writes
    /// `MeterSource::Included` and does not set `use_console`. The next
    /// request draws included SuperGrok period limits. The card shows
    /// `Using limits`. The `Use personal limits` control is gone. The card
    /// does not say included limits are not being drawn. The usage reading
    /// in this fixture is 28% used, so the header shows percent remaining
    /// `limits 72%`. It does not invent `limits 0%` or `$0`. A live
    /// personal session shows `Using personal limits` and does not show
    /// `Using limits`.
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
            screen.contains("Use personal limits")
                && screen.contains("Using SuperGrok dollar credits"),
            "the team-only card offers Use personal limits and names SuperGrok dollar credits:\n{screen}"
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
        let use_limits = phrase_rect(&buf, area, "Use personal limits");
        assert_eq!(painted(&buf, use_limits), "Use personal limits");
        assert!(!use_limits.intersects(close));
        assert!(!use_limits.intersects(esc));
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

        let use_limits = phrase_rect(&buf, area, "Use personal limits");
        let before = screen_of(&buf, area);
        assert!(
            before.contains("Use personal limits"),
            "the click lands on the painted Use personal limits control:\n{before}"
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
            !after.contains("Use personal limits"),
            "the Use personal limits control must not still be sitting there:\n{after}"
        );
        assert!(
            !after.contains("Included limits are not being drawn for the next request."),
            "the accepted choice does not refuse included SuperGrok period limits:\n{after}"
        );
        assert!(
            after.contains("Using limits")
                && after.contains("limits 72%")
                && !after.contains("limits 28%"),
            "the card shows Using limits and the header shows percent remaining:\n{after}"
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
        let use_limits = phrase_rect(&buf, area, "Use personal limits");
        assert_eq!(painted(&buf, use_limits), "Use personal limits");
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
            personal_screen.contains("limits 72%")
                && personal_screen.contains("Using personal limits")
                && !personal_screen.contains("Using limits")
                && !personal_screen.contains("limits 28%"),
            "the header shows percent remaining and the live session shows Using personal limits:\n{personal_screen}"
        );
        assert!(
            !personal_screen.contains("limits 0%") && !personal_screen.contains("$0"),
            "the personal header does not invent limits 0% or $0:\n{personal_screen}"
        );
        clear_console_team_postpaid_cache();
    }

    /// Owed outcome: start from `MeterSource::Included` beside a Team JWT.
    /// Click the credits control. The footer names team postpaid Billing
    /// Credits. Bare `Using credits` does not name that meter. `use_console`
    /// stays false. The pin is not `MeterSource::Console`. The click switches
    /// the next request off included SuperGrok period limits. SuperGrok is paid.
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
            before.contains("Using limits") && before.contains("Use personal limits"),
            "Using limits is selected, so the personal control is Use personal limits:\n{before}"
        );
        assert!(
            !before.contains("Using credits"),
            "the credits choice is not already accepted:\n{before}"
        );
        assert!(
            !before.contains("Use credits"),
            "Use credits is not a painted shortcut:\n{before}"
        );
        let (column, row) = recorded_spend_point(&buf, &agent, "Use personal limits");
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
            "a left click on Use personal limits is handled, got {outcome:?}"
        );
        let pins = load_limits_pins();
        assert!(
            !pins.use_console,
            "Use personal limits keeps use_console false"
        );
        assert_eq!(pins.meter_source, Some(MeterSource::Included));
        assert_ne!(
            pins.meter_source,
            Some(MeterSource::Console),
            "a personal-limits click is not the console meter"
        );
        assert!(
            next_request_draws_included_period_limits(),
            "Use personal limits keeps the next request on included SuperGrok period limits"
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
            after.contains("Using limits"),
            "Using limits stays after the included choice:\n{after}"
        );
        assert!(
            !after.contains("Use credits"),
            "Use credits is not a painted shortcut:\n{after}"
        );
        assert!(
            !after.contains("Using team postpaid Billing Credits"),
            "the footer does not name team postpaid Billing Credits:\n{after}"
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

    /// Named contract: console API credits balance is read with the console
    /// inference key. No management key. The paint does not say
    /// "no management key". A click on `Use personal limits` selects
    /// included SuperGrok period limits. That click is not the console
    /// meter. Personal cents stay unchanged. The card is not required to
    /// repeat the console label or the dollar amount.
    #[tokio::test]
    #[serial_test::serial]
    async fn console_api_credits_balance_and_spend_without_a_management_key() {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Mutex};
        use xai_grok_shell::auth::credentials_store::FORCE_FILE_ENV;
        use xai_grok_shell::auth::limits_pins::{MeterSource, load_limits_pins};
        use xai_grok_shell::auth::{
            LimitsSnapshotDocument, clear_console_team_prepaid_cache,
            fetch_management_into_snapshot, write_limits_snapshot_file,
        };
        use xai_grok_test_support::EnvGuard;

        let inference_key = "inference-key-90035";
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
        state.window.active_tab = LIMITS_TAB;
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
            "the card must not say no management key when the balance is known: {painted}"
        );

        let hit = state
            .window
            .shortcut_hits
            .iter()
            .find(|hit| hit.clickable && hit.id == ACCOUNT_LIMITS_CHOICE_ID)
            .map(|hit| hit.rect)
            .expect("Use personal limits is a button");
        let outcome = handle_limits_mouse(
            &mut state,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            hit.x,
            hit.y,
        );
        assert_eq!(outcome, LimitsModalOutcome::Changed);
        let pins = load_limits_pins();
        assert_eq!(
            pins.meter_source,
            Some(MeterSource::Included),
            "Use personal limits selects included SuperGrok period limits"
        );
        assert!(
            !pins.use_console,
            "Use personal limits does not set use_console"
        );
        assert_ne!(
            pins.meter_source,
            Some(MeterSource::Console),
            "Use personal limits is not the console meter"
        );
        assert_eq!(bal.prepaid_balance_cents, Some(personal_cents));

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
            !gap_paint.contains("$0") && !gap_paint.contains('$'),
            "no balance yet is not an invented dollar amount: {gap_paint}"
        );

        stop.store(true, Ordering::SeqCst);
        let _ = server.join();
    }
    /// Named contract: no management key in the environment or the secret store.
    /// A console inference key is present. A fixture billing response has 76674
    /// cents. The card is not required to repeat that label or `$766.74`. It
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
    /// not the meter in use. Limits mode defaults to limits enabled. Personal
    /// is ordered before Business. Limits and Credits are sections inside
    /// the open tab.
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
        assert_eq!(CARD_TABS[0], "Personal");
        assert_eq!(CARD_TABS[1], "Business");

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
            .find(|line| line.contains("limits 72%"))
            .unwrap_or("")
            .to_string();
        assert!(
            row.contains("limits 72%") && !row.contains("limits 28%"),
            "limits mode paints percent remaining when that meter is in use:\n{header}"
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
            .find(|line| line.contains("Personal") && line.contains("Business"))
            .unwrap_or("");
        let personal_at = tab_line.find("Personal").expect("Personal tab");
        let business_at = tab_line.find("Business").expect("Business tab");
        assert!(
            personal_at < business_at,
            "Personal is ordered before Business:\n{tab_line}"
        );
        assert!(
            card.lines().any(|line| line.trim() == "Limits")
                && card.lines().any(|line| line.trim() == "Credits"),
            "Limits and Credits are sections inside the open tab:\n{card}"
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
            .find(|line| line.contains("Personal") && line.contains("Business"))
            .unwrap_or("");
        assert!(
            dollar_tabs.find("Personal").unwrap_or(usize::MAX)
                < dollar_tabs.find("Business").unwrap_or(0),
            "Personal stays before Business when the meter is SuperGrok dollar credits:\n{dollar_tabs}"
        );
        assert!(
            dollar_card.lines().any(|line| line.trim() == "Limits")
                && dollar_card.lines().any(|line| line.trim() == "Credits"),
            "Limits and Credits stay sections when the meter is SuperGrok dollar credits:\n{dollar_card}"
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
            drawing_header.contains("limits 72%") && !drawing_header.contains("limits 28%"),
            "limits mode paints percent remaining when that meter is in use:\n{drawing_header}"
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
            .find(|line| line.contains("Personal") && line.contains("Business"))
            .unwrap_or("");
        assert!(
            drawing_tabs.find("Personal").unwrap_or(usize::MAX)
                < drawing_tabs.find("Business").unwrap_or(0),
            "Personal is ordered before Business:\n{drawing_tabs}"
        );
        assert!(
            drawing_card.lines().any(|line| line.trim() == "Limits")
                && drawing_card.lines().any(|line| line.trim() == "Credits"),
            "Limits and Credits are sections inside the open tab:\n{drawing_card}"
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
            !unread_card.contains(NOT_READ),
            "an unread included meter stays empty of that sentence:\n{unread_card}"
        );
        assert!(
            !unread_card.contains("0% used"),
            "not available yet is not a live 0%:\n{unread_card}"
        );
        assert!(
            !unread_card.contains("$0"),
            "an unread meter does not invent $0:\n{unread_card}"
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
        let stopped_shortcuts = painted_shortcuts(&stopped);
        assert_eq!(stopped_shortcuts.len(), 3);
        assert_eq!(stopped_shortcuts[0].label, "Use business limits");
        assert_eq!(stopped_shortcuts[1].label, "Using SuperGrok dollar credits");
        assert_eq!(stopped_shortcuts[2].label, "Esc close");
        let credits_card = paint_card(&mut stopped);
        assert_eq!(stopped.window.active_tab, CREDITS_TAB);
        assert!(
            credits_card.contains(NOT_DRAWN),
            "the Credits tab says included limits are not being drawn for the next request:\n{credits_card}"
        );
        assert!(
            !credits_card.contains("Using limits"),
            "the footer must not say Using limits when the card says included limits are not being drawn:\n{credits_card}"
        );
        assert!(
            !credits_card.contains("limits 0%") && !credits_card.contains("$0"),
            "the card must not say limits 0% or $0 beside the not-drawn sentence:\n{credits_card}"
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
            .find(|line| line.contains("Personal") && line.contains("Business"))
            .unwrap_or("");
        assert!(
            credits_tabs.find("Personal").unwrap_or(usize::MAX)
                < credits_tabs.find("Business").unwrap_or(0),
            "Personal stays before Business:\n{credits_tabs}"
        );
        assert!(
            credits_card.lines().any(|line| line.trim() == "Limits")
                && credits_card.lines().any(|line| line.trim() == "Credits"),
            "Limits and Credits are sections inside the open Business tab:\n{credits_card}"
        );

        stopped.window.active_tab = LIMITS_TAB;
        let limits_card = paint_card(&mut stopped);
        assert!(
            limits_card.contains(NOT_DRAWN),
            "the Limits tab says included limits are not being drawn for the next request:\n{limits_card}"
        );
        assert!(
            !limits_card.contains(NOT_READ),
            "the Limits tab stays empty of the unread allowance sentence:\n{limits_card}"
        );
        assert!(
            !limits_card.contains("Using limits")
                && !limits_card.contains("limits 0%")
                && !limits_card.contains("$0"),
            "Using limits, limits 0%, and $0 stay off while the card says included limits are not being drawn:\n{limits_card}"
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
            personal_header.contains("limits 72%") && !personal_header.contains("limits 28%"),
            "limits mode paints percent remaining when that meter is in use:\n{personal_header}"
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
    /// control. Opening it shows Personal before Business. Limits and Credits
    /// are sections inside the open tab. The control does not say `limits 0%`
    /// or `limits N%`.
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
            .find(|line| line.contains("Personal") && line.contains("Business"))
            .unwrap_or("");
        let personal_at = tab_line.find("Personal").expect("Personal tab");
        let business_at = tab_line.find("Business").expect("Business tab");
        assert!(
            personal_at < business_at,
            "opening the control shows Personal before Business:\n{tab_line}"
        );
        assert!(
            card.lines().any(|line| line.trim() == "Limits")
                && card.lines().any(|line| line.trim() == "Credits"),
            "Limits and Credits are sections inside the open tab:\n{card}"
        );
        assert_eq!(state.window.active_tab, LIMITS_TAB);
    }

    const CELL_PERSONAL_TOKEN: &str = "tok-personal-cell";
    const CELL_TEAM_TOKEN: &str = "tok-team-cell";
    const CELL_PERSONAL_ID: &str = "u-personal-cell";
    const CELL_BUSINESS_ID: &str = "team-cell";
    const CELL_PERSONAL_PCT: f64 = 28.0;
    const CELL_BUSINESS_PCT: f64 = 41.0;
    const CELL_PERSONAL_CENTS: i64 = 8_765;
    const CELL_BUSINESS_CENTS: i64 = 2_199;
    const CELL_CONSOLE_CENTS: i64 = 15_420;
    const CELL_POSTPAID_CENTS: i64 = 4_703;
    const CELL_POSTPAID_DECOY_CENTS: i64 = 44_297;

    struct SeededHome {
        home: tempfile::TempDir,
        _env: xai_grok_test_support::EnvGuard,
    }

    fn seed_four_cells_for_card_click(config_toml: Option<&str>) -> SeededHome {
        use xai_grok_shell::auth::{
            AuthMode, GrokAuth, LimitsSnapshotDocument, LimitsSnapshotIdentity,
            LimitsSnapshotManagement, POLL_OUTCOME_OK, clear_console_team_postpaid_cache,
            clear_included_billing_cache, upsert_supergrok_session, write_limits_snapshot_file,
        };

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        clear_included_billing_cache();
        clear_console_team_postpaid_cache();
        if let Some(body) = config_toml {
            std::fs::write(home.path().join("config.toml"), body).expect("config.toml");
        }
        let live = chrono::Utc::now() + chrono::Duration::days(7);
        let base = "https://auth.x.ai::four-cell-click";
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: CELL_PERSONAL_TOKEN.into(),
                auth_mode: AuthMode::Oidc,
                user_id: CELL_PERSONAL_ID.into(),
                principal_type: Some("User".into()),
                principal_id: Some(CELL_PERSONAL_ID.into()),
                team_id: None,
                expires_at: Some(live),
                ..GrokAuth::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: CELL_TEAM_TOKEN.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-team-cell".into(),
                principal_type: Some("Team".into()),
                principal_id: Some(CELL_BUSINESS_ID.into()),
                team_id: Some(CELL_BUSINESS_ID.into()),
                expires_at: Some(live),
                ..GrokAuth::default()
            },
        );
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth.json"),
        )
        .expect("write auth.json");
        let identity = |identity_id: &str, usage_pct: f64, cents: i64| LimitsSnapshotIdentity {
            identity_id: identity_id.to_owned(),
            usage_pct: Some(usage_pct),
            period_end: None,
            period_type: None,
            dollar_credits_cents: Some(cents),
            grok_build_usage_pct: None,
            is_unified_billing_user: None,
            poll_outcome: POLL_OUTCOME_OK.to_owned(),
        };
        let mut doc = LimitsSnapshotDocument::empty(1_700_000_000_000);
        doc.identities.push(identity(
            CELL_PERSONAL_ID,
            CELL_PERSONAL_PCT,
            CELL_PERSONAL_CENTS,
        ));
        doc.identities.push(identity(
            CELL_BUSINESS_ID,
            CELL_BUSINESS_PCT,
            CELL_BUSINESS_CENTS,
        ));
        doc.management = Some(LimitsSnapshotManagement {
            team_id: Some("mgmt-cell-decoy".into()),
            prepaid_cents: Some(CELL_CONSOLE_CENTS),
            billing_credits_cents: Some(CELL_POSTPAID_CENTS),
            ..Default::default()
        });
        write_limits_snapshot_file(home.path(), &doc).expect("four meter rows");
        SeededHome { home, _env: env }
    }

    fn find_phrase(buf: &Buffer, area: Rect, phrase: &str) -> Option<(u16, u16)> {
        for y in area.y..area.y.saturating_add(area.height) {
            let mut row = String::new();
            for x in area.x..area.x.saturating_add(area.width) {
                row.push_str(buf[(x, y)].symbol());
            }
            if let Some(byte) = row.find(phrase) {
                let x_off = row[..byte].chars().count() as u16;
                return Some((area.x.saturating_add(x_off), y));
            }
        }
        None
    }

    fn assert_other_three_snapshot_values(home: &std::path::Path) {
        let doc = xai_grok_shell::auth::read_limits_snapshot_file(home).expect("snapshot stored");
        let personal = doc
            .identities
            .iter()
            .find(|row| row.identity_id == CELL_PERSONAL_ID)
            .expect("personal row");
        let business = doc
            .identities
            .iter()
            .find(|row| row.identity_id == CELL_BUSINESS_ID)
            .expect("business row");
        assert_eq!(personal.usage_pct, Some(CELL_PERSONAL_PCT));
        assert_eq!(personal.dollar_credits_cents, Some(CELL_PERSONAL_CENTS));
        assert_eq!(business.usage_pct, Some(CELL_BUSINESS_PCT));
        assert_eq!(business.dollar_credits_cents, Some(CELL_BUSINESS_CENTS));
        let management = doc.management.expect("management meters stay stored");
        assert_eq!(management.prepaid_cents, Some(CELL_CONSOLE_CENTS));
        assert_eq!(management.billing_credits_cents, Some(CELL_POSTPAID_CENTS));
    }

    fn sampler_starting_on(token: &str) -> xai_grok_shell::sampling::SamplerConfig {
        use xai_grok_shell::sampling::SamplerConfig;
        SamplerConfig {
            api_key: Some(token.into()),
            failover_api_keys: vec![
                CELL_PERSONAL_TOKEN.into(),
                CELL_TEAM_TOKEN.into(),
                "console-not-this-cell".into(),
            ],
            base_url: "https://cli-chat-proxy.grok.com/v1".into(),
            model: "grok-4".into(),
            session_identity_key: Some(token.into()),
            failover_base_url: Some("https://api.x.ai/v1".into()),
            session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
            ..Default::default()
        }
    }

    /// Owed outcome: when the personal identity row's pollOutcome is
    /// `reauth`, the Personal tab contains this exact sentence and no other
    /// wording for that failure. `auth` is not this case.
    #[test]
    #[serial_test::serial]
    fn personal_tab_says_run_grok_oss_login_when_the_personal_refresh_token_is_rejected() {
        use xai_grok_shell::auth::{
            AuthMode, GrokAuth, LimitsSnapshotDocument, LimitsSnapshotIdentity,
            clear_included_billing_cache, upsert_supergrok_session, write_limits_snapshot_file,
        };

        let owed = "Personal SuperGrok login expired. Run grok-oss login for the personal account. That login keeps the business login.";
        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        clear_included_billing_cache();
        clear_account_billing_cache();
        std::fs::write(
            home.path().join("config.toml"),
            "[auth]\npreferred_method = \"oidc\"\n",
        )
        .expect("preferred oidc");
        let live = chrono::Utc::now() + chrono::Duration::days(7);
        let base = "https://auth.x.ai::personal-reauth-tab";
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "personal-reauth-session".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-personal-reauth".into(),
                principal_type: Some("User".into()),
                principal_id: Some("u-personal-reauth".into()),
                team_id: None,
                expires_at: Some(live),
                ..GrokAuth::default()
            },
        );
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth json"),
        )
        .expect("write auth.json");
        let mut doc = LimitsSnapshotDocument::empty(1_700_000_000_000);
        doc.identities.push(LimitsSnapshotIdentity {
            identity_id: "u-personal-reauth".into(),
            usage_pct: None,
            period_end: None,
            period_type: None,
            dollar_credits_cents: None,
            grok_build_usage_pct: None,
            is_unified_billing_user: None,
            poll_outcome: "reauth".to_owned(),
        });
        write_limits_snapshot_file(home.path(), &doc).expect("snapshot");

        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-10-06T22:08:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let bal = weekly_bal(99.0, end);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let mut state = LimitsModalState::new(snap);
        assert_eq!(
            state.window.active_tab, LIMITS_TAB,
            "Personal is the open tab"
        );
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);
        render_limits_modal(&mut buf, area, &mut state, &theme, false, now);
        let card = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let collapsed = card.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            collapsed.contains(owed),
            "the Personal tab must contain `{owed}` when pollOutcome is reauth:\n{card}"
        );
        assert!(
            !collapsed.contains("Personal SuperGrok has no session on this process."),
            "the Personal tab must not use other wording for that failure:\n{card}"
        );
    }

    /// Owed outcome: the Personal tab does not list the four meter cells.
    /// It does not show Personal limits, Personal credits, Business limits,
    /// Business credits, Team postpaid Billing Credits, or Console API
    /// credits. It does not show `limits 28%`, `limits 41%`, `$87.65`,
    /// `$21.99`, `$442.97`, `limits 0%`, or `$0`. The stored poll percent
    /// is not copied onto the section. Empty included usage and prepaid
    /// stay empty, because a fixture token is not a session JWT. The footer
    /// still contains `Use personal limits`. The card still contains
    /// `Esc close` and does not contain `approve | comment | revise | exit`.
    /// A failed poll still does not copy a percent onto the section.
    /// SuperGrok is paid.
    #[test]
    #[serial_test::serial]
    fn limits_card_lists_four_cells_and_does_not_copy_one_sessions_reading_onto_the_other() {
        use xai_grok_shell::auth::{
            POLL_OUTCOME_NETWORK, POLL_OUTCOME_OK, read_limits_snapshot_file,
            remember_supergrok_dollar_credits, remember_supergrok_included_billing,
            write_limits_snapshot_file,
        };

        struct ClearBillingCache;
        impl Drop for ClearBillingCache {
            fn drop(&mut self) {
                xai_grok_shell::auth::clear_included_billing_cache();
                xai_grok_shell::auth::clear_console_team_postpaid_cache();
            }
        }

        let _clear = ClearBillingCache;
        let seeded = seed_four_cells_for_card_click(None);
        let mut doc = read_limits_snapshot_file(seeded.home.path()).expect("four meter rows");
        let management = doc.management.as_mut().expect("management meters");
        management.billing_credits_cents = Some(CELL_POSTPAID_DECOY_CENTS);
        management.billing_credits_card = serde_json::from_value(serde_json::json!("fetched"))
            .expect("fetched Billing Credits card");
        write_limits_snapshot_file(seeded.home.path(), &doc).expect("fetched postpaid card");

        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-10-06T22:08:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut bal = weekly_bal(99.0, end);
        bal.prepaid_balance_cents = Some(1_250);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let mut state = LimitsModalState::new(snap);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 40);
        let paint = |state: &mut LimitsModalState| -> String {
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

        assert_eq!(state.window.active_tab, LIMITS_TAB);
        let card = paint(&mut state);
        assert!(
            !card.contains("approve | comment | revise | exit"),
            "the plan-approval footer is not this card:\n{card}"
        );
        assert!(card.contains("Esc close"), "the card footer stays:\n{card}");
        assert!(
            card.contains("Use personal limits"),
            "the Personal tab footer still contains Use personal limits:\n{card}"
        );
        for absent in [
            "Personal limits",
            "Personal credits",
            "Business limits",
            "Business credits",
            "Team postpaid Billing Credits",
            "Console API credits",
            "limits 28%",
            "limits 41%",
            "$87.65",
            "$21.99",
            "$442.97",
            "limits 0%",
            "$0",
        ] {
            assert!(
                !card.contains(absent),
                "the Personal tab does not show {absent}:\n{card}"
            );
        }
        assert!(
            !card.contains("Included usage:") && !card.contains("Prepaid balance:"),
            "empty included usage and prepaid stay empty. A fixture token is not a session JWT, and the stored poll percent is not copied onto the section:\n{card}"
        );

        let mut copied = read_limits_snapshot_file(seeded.home.path()).expect("rows stay stored");
        for row in &mut copied.identities {
            if row.identity_id == CELL_PERSONAL_ID {
                row.usage_pct = Some(24.0);
                row.dollar_credits_cents = Some(CELL_PERSONAL_CENTS);
                row.poll_outcome = POLL_OUTCOME_OK.to_owned();
            }
            if row.identity_id == CELL_BUSINESS_ID {
                row.usage_pct = Some(24.0);
                row.dollar_credits_cents = Some(CELL_BUSINESS_CENTS);
                row.poll_outcome = POLL_OUTCOME_NETWORK.to_owned();
            }
        }
        write_limits_snapshot_file(seeded.home.path(), &copied).expect("failed business poll");
        remember_supergrok_included_billing(CELL_PERSONAL_ID, 41.0, None, None);
        remember_supergrok_dollar_credits(CELL_PERSONAL_ID, CELL_BUSINESS_CENTS);
        remember_supergrok_included_billing(CELL_BUSINESS_ID, 24.0, None, None);
        remember_supergrok_dollar_credits(CELL_BUSINESS_ID, CELL_BUSINESS_CENTS);

        let failed = paint(&mut state);
        assert!(
            failed.contains("Use personal limits"),
            "the Personal tab footer still contains Use personal limits after the failed poll:\n{failed}"
        );
        for absent in [
            "Personal limits",
            "Personal credits",
            "Business limits",
            "Business credits",
            "Team postpaid Billing Credits",
            "Console API credits",
            "limits 24%",
            "limits 28%",
            "limits 41%",
            "$87.65",
            "$21.99",
            "$442.97",
            "limits 0%",
            "$0",
            "Included usage:",
            "Prepaid balance:",
        ] {
            assert!(
                !failed.contains(absent),
                "a failed poll does not copy a stored percent onto the Personal tab ({absent}):\n{failed}"
            );
        }
    }

    /// A click on `Use personal limits` writes both pins through the card.
    /// The status figure is percent remaining `limits 72%`.
    #[test]
    #[serial_test::serial]
    fn clicking_personal_limits_writes_the_personal_included_pin() {
        use crate::actions::ActionRegistry;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::scrollback::render::ScratchBuffer;
        use xai_grok_shell::auth::SupergrokIdentityPin;
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, load_limits_pins,
            next_request_draws_included_period_limits, pinned_supergrok_row,
        };

        let owed = "A click on Use personal limits selects included SuperGrok period limits for the personal session. The status figure is percent remaining limits 72%. The personal row usage is 28 percent used. The next request sends the personal session. use_console is false. Team postpaid Billing Credits are not the bearer.";
        let seeded = seed_four_cells_for_card_click(None);
        click_exact_footer_phrase("Use personal limits")
            .unwrap_or_else(|err| panic!("owed: {owed} {err}"));
        let pins = load_limits_pins();
        assert!(
            seeded.home.path().join("limits_pins.json").exists(),
            "owed: {owed}"
        );
        assert_eq!(
            pins.meter_source,
            Some(MeterSource::Included),
            "owed: {owed}"
        );
        assert_eq!(
            pins.supergrok_identity,
            Some(SupergrokIdentityPin::Personal),
            "owed: {owed}"
        );
        assert!(!pins.use_console, "owed: {owed}");
        assert!(pins.stay_supergrok, "owed: {owed}");
        assert!(next_request_draws_included_period_limits(), "owed: {owed}");
        let mut config = sampler_starting_on(CELL_TEAM_TOKEN);
        apply_limits_pins_to_sampler_config(&mut config);
        assert_eq!(
            config.api_key.as_deref(),
            Some(CELL_PERSONAL_TOKEN),
            "owed: {owed}"
        );
        assert_ne!(config.api_key.as_deref(), Some("console-not-this-cell"));
        assert!(
            config.base_url.contains("cli-chat-proxy"),
            "owed: {owed} base_url={}",
            config.base_url
        );
        assert!(
            !config.base_url.contains("api.x.ai"),
            "owed: {owed} The click does not hop to the console API key. base_url={}",
            config.base_url
        );
        let row = pinned_supergrok_row().unwrap_or_else(|| panic!("owed: {owed}"));
        assert_eq!(row.usage_pct, Some(CELL_PERSONAL_PCT), "owed: {owed}");
        assert_ne!(row.usage_pct, Some(99.0), "owed: {owed}");
        assert_ne!(row.usage_pct, Some(CELL_BUSINESS_PCT), "owed: {owed}");
        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut decoy = weekly_bal(99.0, end);
        decoy.usage_pct = 99.0;
        decoy.included_usage_known = true;
        crate::appearance::cache::set_hide_header(false);
        let mut agent = make_agent();
        agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
        agent.credit_balance = Some(decoy);
        let area = Rect::new(0, 0, 140, 40);
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
        let screen = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| header[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            screen.contains("limits 72%")
                && !screen.contains("limits 28%")
                && !screen.contains("Limits and Credits")
                && !screen.contains("limits 0%")
                && !screen.contains("$0"),
            "owed: {owed} the status figure is percent remaining:\n{screen}"
        );
        assert_other_three_snapshot_values(seeded.home.path());
    }

    /// Personal credits are not on the Personal tab, so this test does not
    /// write a dollar pin.
    #[test]
    #[serial_test::serial]
    fn clicking_personal_credits_writes_the_personal_dollar_pin() {
        let owed = "The Personal tab does not list Personal credits, Business credits, Team postpaid Billing Credits, or Console API credits. It does not show limits 0% or $0. The footer still contains Use personal limits. This test does not write a dollar pin, because that cell is not on the tab.";
        assert_personal_tab_omits_removed_meter_cells(owed);
    }

    /// A click on `Use business limits` writes both pins through the card.
    #[test]
    #[serial_test::serial]
    fn clicking_business_limits_writes_the_team_included_pin() {
        use xai_grok_shell::auth::SupergrokIdentityPin;
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, load_limits_pins,
            next_request_draws_included_period_limits, pinned_supergrok_row,
        };

        let owed = "A click on Use business limits selects the Team session's included SuperGrok period limits. The stored usage percent on that row is 41. The next request sends the Team session. use_console is false. The click does not hop to the console API key.";
        let seeded = seed_four_cells_for_card_click(None);
        click_exact_footer_phrase("Use business limits")
            .unwrap_or_else(|err| panic!("owed: {owed} {err}"));
        let pins = load_limits_pins();
        assert_eq!(
            pins.meter_source,
            Some(MeterSource::Included),
            "owed: {owed}"
        );
        assert_eq!(
            pins.supergrok_identity,
            Some(SupergrokIdentityPin::Business),
            "owed: {owed}"
        );
        assert!(!pins.use_console, "owed: {owed}");
        assert!(pins.stay_supergrok, "owed: {owed}");
        assert!(next_request_draws_included_period_limits(), "owed: {owed}");
        let mut config = sampler_starting_on(CELL_PERSONAL_TOKEN);
        apply_limits_pins_to_sampler_config(&mut config);
        assert_eq!(
            config.api_key.as_deref(),
            Some(CELL_TEAM_TOKEN),
            "owed: {owed}"
        );
        assert_ne!(
            config.api_key.as_deref(),
            Some(CELL_PERSONAL_TOKEN),
            "owed: {owed}"
        );
        assert_ne!(config.api_key.as_deref(), Some("console-not-this-cell"));
        assert!(
            config.base_url.contains("cli-chat-proxy"),
            "owed: {owed} base_url={}",
            config.base_url
        );
        assert!(
            !config.base_url.contains("api.x.ai"),
            "owed: {owed} The click does not hop to the console API key."
        );
        let row = pinned_supergrok_row().unwrap_or_else(|| panic!("owed: {owed}"));
        assert_eq!(row.usage_pct, Some(CELL_BUSINESS_PCT), "owed: {owed}");
        assert_ne!(row.usage_pct, Some(CELL_PERSONAL_PCT), "owed: {owed}");
        assert_ne!(row.usage_pct, Some(99.0), "owed: {owed}");
        assert_other_three_snapshot_values(seeded.home.path());
    }

    /// Business credits are not on the Personal tab, so this test does not
    /// write a dollar pin.
    #[test]
    #[serial_test::serial]
    fn clicking_business_credits_writes_the_team_dollar_pin() {
        let owed = "The Personal tab does not list Personal credits, Business credits, Team postpaid Billing Credits, or Console API credits. It does not show limits 0% or $0. The footer still contains Use personal limits. This test does not write a dollar pin, because that cell is not on the tab.";
        assert_personal_tab_omits_removed_meter_cells(owed);
    }

    fn assert_personal_tab_omits_removed_meter_cells(owed: &str) {
        let _seeded = seed_four_cells_for_card_click(None);
        let area = Rect::new(0, 0, 120, 40);
        let mut state = fresh_card_state();
        let buf = render_seeded_card(&mut state, area);
        assert_eq!(
            state.window.active_tab, LIMITS_TAB,
            "owed: {owed} the default tab is Personal"
        );
        let card = painted_lines(&buf, area).join("\n");
        assert!(
            card.contains("Use personal limits"),
            "owed: {owed} the footer still contains Use personal limits:\n{card}"
        );
        for absent in [
            "Personal credits",
            "Business credits",
            "Team postpaid Billing Credits",
            "Console API credits",
            "limits 0%",
            "$0",
        ] {
            assert!(
                !card.contains(absent),
                "owed: {owed} the Personal tab does not show {absent}:\n{card}"
            );
        }
    }

    /// preferred_method api_key still lets a Business limits click write both pins.
    #[test]
    #[serial_test::serial]
    fn clicking_business_limits_writes_both_pins_when_preferred_method_is_api_key() {
        use xai_grok_shell::auth::SupergrokIdentityPin;
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, load_limits_pins,
            next_request_draws_included_period_limits, pinned_supergrok_row,
        };

        let owed = "preferred_method is api_key. A click on Use business limits still writes the meter pin and the identity pin. use_console stays false. The click is not blocked by preferred_method.";
        let seeded =
            seed_four_cells_for_card_click(Some("[auth]\npreferred_method = \"api_key\"\n"));
        click_exact_footer_phrase("Use business limits")
            .unwrap_or_else(|err| panic!("owed: {owed} {err}"));
        let config_text = std::fs::read_to_string(seeded.home.path().join("config.toml"))
            .expect("config.toml stays");
        assert!(
            config_text.contains("preferred_method = \"api_key\""),
            "owed: {owed}"
        );
        let pins = load_limits_pins();
        assert!(
            seeded.home.path().join("limits_pins.json").exists(),
            "owed: {owed} The click is not blocked by preferred_method."
        );
        assert_eq!(
            pins.meter_source,
            Some(MeterSource::Included),
            "owed: {owed}"
        );
        assert_eq!(
            pins.supergrok_identity,
            Some(SupergrokIdentityPin::Business),
            "owed: {owed}"
        );
        assert!(!pins.use_console, "owed: {owed}");
        assert!(pins.stay_supergrok, "owed: {owed}");
        assert!(next_request_draws_included_period_limits(), "owed: {owed}");
        let mut config = sampler_starting_on(CELL_PERSONAL_TOKEN);
        apply_limits_pins_to_sampler_config(&mut config);
        assert_eq!(
            config.api_key.as_deref(),
            Some(CELL_TEAM_TOKEN),
            "owed: {owed}"
        );
        assert_ne!(config.api_key.as_deref(), Some("console-not-this-cell"));
        assert!(
            config.base_url.contains("cli-chat-proxy"),
            "owed: {owed} The click does not hop to the console API key."
        );
        assert!(
            !config.base_url.contains("api.x.ai"),
            "owed: {owed} The click does not hop to the console API key. base_url={}",
            config.base_url
        );
        let row = pinned_supergrok_row().unwrap_or_else(|| panic!("owed: {owed}"));
        assert_eq!(row.usage_pct, Some(CELL_BUSINESS_PCT), "owed: {owed}");
        assert_other_three_snapshot_values(seeded.home.path());
    }

    fn painted_lines(buf: &Buffer, area: Rect) -> Vec<String> {
        (area.y..area.y.saturating_add(area.height))
            .map(|y| {
                (area.x..area.x.saturating_add(area.width))
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    fn rect_glyphs(buf: &Buffer, rect: Rect) -> String {
        (0..rect.width)
            .map(|dx| buf[(rect.x.saturating_add(dx), rect.y)].symbol())
            .collect()
    }

    fn tab_labels(buf: &Buffer, state: &LimitsModalState) -> Vec<String> {
        state
            .window
            .tab_rects
            .iter()
            .map(|rect| {
                rect.as_ref()
                    .map(|rect| rect_glyphs(buf, *rect).trim().to_string())
                    .unwrap_or_default()
            })
            .collect()
    }

    fn line_is_chrome_or_footer(line: &str) -> bool {
        const MARKERS: [&str; 11] = [
            "Esc close",
            "Use personal limits",
            "Use business limits",
            "Using personal limits",
            "Using business limits",
            "Use personal credits",
            "Use business credits",
            "Use limits",
            "Using limits",
            "Use credits",
            "Using credits",
        ];
        line.chars()
            .any(|glyph| matches!(glyph, '─' | '│' | '┌' | '┐' | '└' | '┘' | '├' | '┤'))
            || MARKERS.iter().any(|marker| line.contains(marker))
    }

    /// A section heading is the word Limits or Credits. Cell lines such as
    /// `Personal limits` and footer labels are not headings.
    fn section_heading_name(line: &str) -> Option<&'static str> {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("use ")
            || lower.starts_with("using ")
            || lower.starts_with("personal ")
            || lower.starts_with("business ")
            || lower.starts_with("included ")
            || lower.starts_with("console ")
            || lower.starts_with("team ")
        {
            return None;
        }
        if lower == "limits" || lower.starts_with("limits ") || lower.starts_with("limits:") {
            return Some("Limits");
        }
        if lower == "credits" || lower.starts_with("credits ") || lower.starts_with("credits:") {
            return Some("Credits");
        }
        None
    }

    fn section_text(lines: &[String], name: &str) -> Option<String> {
        let mut body = String::new();
        let mut taking = false;
        for line in lines {
            if let Some(heading) = section_heading_name(line) {
                if taking {
                    break;
                }
                taking = heading == name;
            }
            if taking {
                body.push_str(line);
                body.push('\n');
            }
        }
        taking.then_some(body)
    }

    fn account_body_lines(buf: &Buffer, area: Rect, state: &LimitsModalState) -> Vec<String> {
        let tab_rows: Vec<u16> = state
            .window
            .tab_rects
            .iter()
            .filter_map(|rect| rect.map(|rect| rect.y))
            .collect();
        painted_lines(buf, area)
            .into_iter()
            .enumerate()
            .filter(|(index, line)| {
                let y = area.y.saturating_add(*index as u16);
                !tab_rows.contains(&y) && !line_is_chrome_or_footer(line)
            })
            .map(|(_, line)| line)
            .collect()
    }

    fn render_seeded_card(state: &mut LimitsModalState, area: Rect) -> Buffer {
        let theme = Theme::default();
        let now = DateTime::parse_from_rfc3339("2026-10-06T22:08:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut buf = Buffer::empty(area);
        render_limits_modal(&mut buf, area, state, &theme, false, now);
        buf
    }

    fn fresh_card_state() -> LimitsModalState {
        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut bal = weekly_bal(99.0, end);
        bal.included_usage_known = true;
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        LimitsModalState::new(snap)
    }

    fn select_account_tab(state: &mut LimitsModalState, buf: &Buffer, account: &str) -> bool {
        let labels = tab_labels(buf, state);
        let Some(index) = labels.iter().position(|label| label == account) else {
            return false;
        };
        if state.window.active_tab == index {
            return true;
        }
        let Some(rect) = state.window.tab_rects.get(index).and_then(|rect| *rect) else {
            return false;
        };
        let _ = modal_window::handle_modal_mouse(
            &mut state.window,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            rect.x,
            rect.y,
        );
        true
    }

    /// Left click on the exact footer phrase. This does not call
    /// `select_supergrok_meter_cell`, `apply_use_personal`, `apply_use_business`,
    /// or `save_limits_pins`. A cell labeled `Personal limits` is not this button.
    fn click_exact_footer_phrase(phrase: &str) -> Result<(), String> {
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::scrollback::render::ScratchBuffer;
        use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

        let account = if phrase == "Use personal limits" {
            "Personal"
        } else if phrase == "Use business limits" {
            "Business"
        } else {
            return Err(format!(
                "the footer phrase `{phrase}` is not a personal or business limits button"
            ));
        };
        let registry = crate::actions::ActionRegistry::defaults();
        let area = Rect::new(0, 0, 140, 40);
        let _theme = crate::theme::cache::pin_theme();
        crate::appearance::cache::set_hide_header(false);
        let mut agent = make_agent();
        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut bal = weekly_bal(99.0, end);
        bal.included_usage_known = true;
        agent.credit_balance = Some(bal.clone());
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        agent.active_modal = Some(crate::views::modal::ActiveModal::Limits {
            state: Box::new(LimitsModalState::new(snap)),
        });
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
        let mut buf = draw(&mut agent);
        if find_phrase(&buf, area, phrase).is_none() {
            // Show the account tab without writing the pin. A tab click
            // through handle_input would apply that account's included
            // pin and replace `Use business limits` with `Using business
            // limits`. The pin-writer click is the phrase itself.
            if let Some(crate::views::modal::ActiveModal::Limits { state }) =
                agent.active_modal.as_mut()
            {
                let index = if account == "Business" {
                    CREDITS_TAB
                } else {
                    LIMITS_TAB
                };
                state.window.active_tab = index;
            }
            buf = draw(&mut agent);
        }
        let Some((column, row)) = find_phrase(&buf, area, phrase) else {
            let tabs = match agent.active_modal.as_ref() {
                Some(crate::views::modal::ActiveModal::Limits { state }) => tab_labels(&buf, state),
                _ => Vec::new(),
            };
            return Err(format!(
                "the footer button `{phrase}` is not on the card. Tabs are {tabs:?}. This click does not call the pin writer, and it does not click the `{account} limits` cell."
            ));
        };
        agent.handle_input(
            &Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            }),
            &registry,
        );
        Ok(())
    }

    fn assert_included_session_pin(
        identity: xai_grok_shell::auth::SupergrokIdentityPin,
        token: &str,
        owed: &str,
    ) {
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, load_limits_pins,
            next_request_draws_included_period_limits,
        };

        let pins = load_limits_pins();
        assert_eq!(
            pins.meter_source,
            Some(MeterSource::Included),
            "owed: {owed}"
        );
        assert_eq!(pins.supergrok_identity, Some(identity), "owed: {owed}");
        assert!(!pins.use_console, "owed: {owed}");
        assert!(pins.stay_supergrok, "owed: {owed}");
        assert!(next_request_draws_included_period_limits(), "owed: {owed}");
        let starting = if token == CELL_PERSONAL_TOKEN {
            CELL_TEAM_TOKEN
        } else {
            CELL_PERSONAL_TOKEN
        };
        let mut config = sampler_starting_on(starting);
        apply_limits_pins_to_sampler_config(&mut config);
        assert_eq!(config.api_key.as_deref(), Some(token), "owed: {owed}");
        assert_ne!(
            config.api_key.as_deref(),
            Some("console-not-this-cell"),
            "owed: {owed}"
        );
        assert!(
            config.base_url.contains("cli-chat-proxy"),
            "owed: {owed} base_url={}",
            config.base_url
        );
        assert!(
            !config.base_url.contains("api.x.ai"),
            "owed: {owed} The click keeps the SuperGrok session on the proxy."
        );
    }

    /// The open card shows Personal and Business. Each tab has a Limits
    /// section and a Credits section. Personal's footer reads `Use personal
    /// limits`. Business's footer reads `Use business limits`.
    #[test]
    #[serial_test::serial]
    fn open_limits_card_shows_personal_and_business_tabs_with_limits_and_credits_sections() {
        let _seeded = seed_four_cells_for_card_click(None);
        let area = Rect::new(0, 0, 120, 40);
        let mut state = fresh_card_state();
        let mut misses = Vec::new();
        let mut buf = render_seeded_card(&mut state, area);
        let tabs = tab_labels(&buf, &state);
        if tabs != ["Personal", "Business"] {
            misses.push(format!(
                "the open card shows tabs Personal and Business. Painted tabs are {tabs:?}."
            ));
        }
        for account in ["Personal", "Business"] {
            buf = render_seeded_card(&mut state, area);
            if !select_account_tab(&mut state, &buf, account) {
                misses.push(format!("the {account} tab is not on the card."));
            }
            buf = render_seeded_card(&mut state, area);
            let card = painted_lines(&buf, area).join("\n");
            let button = match account {
                "Personal" => "Use personal limits",
                _ => "Use business limits",
            };
            let absent = match account {
                "Personal" => "Use personal credits",
                _ => "Use business credits",
            };
            if !card.contains(button) {
                misses.push(format!(
                    "the {account} tab footer reads `{button}`. The card does not show that button."
                ));
            }
            if card.contains(absent) {
                misses.push(format!(
                    "the card shows `{absent}`. There is no `{absent}` button."
                ));
            }
            let body = account_body_lines(&buf, area, &state);
            if section_text(&body, "Limits").is_none() {
                misses.push(format!(
                    "the {account} tab has a Limits section. That section is not on the card."
                ));
            }
            if section_text(&body, "Credits").is_none() {
                misses.push(format!(
                    "the {account} tab has a Credits section. That section is not on the card."
                ));
            }
        }
        assert!(misses.is_empty(), "{}", misses.join("\n"));
    }

    struct DualHeavyHome {
        _home: tempfile::TempDir,
        _env: xai_grok_test_support::EnvGuard,
        now: DateTime<Utc>,
        personal_end: DateTime<Utc>,
        business_end: DateTime<Utc>,
        personal_id: String,
        business_id: String,
    }

    impl Drop for DualHeavyHome {
        fn drop(&mut self) {
            clear_account_billing_cache();
            xai_grok_shell::auth::clear_included_billing_cache();
        }
    }

    fn remember_ready_account_billing(identity_id: &str, fields: AccountBillingFields) {
        let home = limits_reading_grok_home();
        let key = format!("{}|{}", home.display(), identity_id);
        if let Ok(mut guard) = account_billing_cache().lock() {
            guard.insert(key, CachedAccountBilling::Ready(fields));
        }
    }

    /// Personal Heavy used 100% remaining 0, SuperGrok dollar credits known
    /// zero, nextReset T0. Business Heavy used 20% remaining 80, SuperGrok
    /// dollar credits known zero, nextReset T1. `now` makes T0 `6d 3h` and
    /// T1 `6d 11h`. Live cache holds usage and prepaid. Identity `period_end`
    /// is the snapshot fallback.
    fn seed_dual_heavy_identities() -> DualHeavyHome {
        use xai_grok_shell::auth::{
            AuthMode, GrokAuth, LimitsSnapshotDocument, LimitsSnapshotIdentity, POLL_OUTCOME_OK,
            upsert_supergrok_session, write_limits_snapshot_file,
        };

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        clear_account_billing_cache();
        xai_grok_shell::auth::clear_included_billing_cache();

        let now = DateTime::parse_from_rfc3339("2026-10-16T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let personal_end = now + chrono::Duration::days(6) + chrono::Duration::hours(3);
        let business_end = now + chrono::Duration::days(6) + chrono::Duration::hours(11);

        let live = now + chrono::Duration::days(14);
        let base = "https://auth.x.ai::dual-heavy";
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "personal.heavy.jwt".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-personal-heavy".into(),
                principal_type: Some("User".into()),
                principal_id: Some("u-personal-heavy".into()),
                team_id: None,
                expires_at: Some(live),
                ..GrokAuth::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "business.heavy.jwt".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-business-heavy".into(),
                principal_type: Some("Team".into()),
                principal_id: Some("team-heavy".into()),
                team_id: Some("team-heavy".into()),
                expires_at: Some(live),
                ..GrokAuth::default()
            },
        );
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth.json"),
        )
        .expect("write auth.json");

        let personal_id =
            poll_target_for_role(xai_grok_shell::auth::SupergrokAccountRole::Personal)
                .expect("personal poll target")
                .identity_id;
        let business_id =
            poll_target_for_role(xai_grok_shell::auth::SupergrokAccountRole::Business)
                .expect("business poll target")
                .identity_id;

        let identity =
            |identity_id: &str, usage_pct: f64, end: DateTime<Utc>| LimitsSnapshotIdentity {
                identity_id: identity_id.to_owned(),
                usage_pct: Some(usage_pct),
                period_end: Some(end.to_rfc3339()),
                period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
                dollar_credits_cents: Some(0),
                grok_build_usage_pct: None,
                is_unified_billing_user: None,
                poll_outcome: POLL_OUTCOME_OK.to_owned(),
            };
        let mut doc = LimitsSnapshotDocument::empty(1_700_000_000_000);
        doc.identities
            .push(identity(&personal_id, 100.0, personal_end));
        doc.identities
            .push(identity(&business_id, 20.0, business_end));
        write_limits_snapshot_file(home.path(), &doc).expect("dual heavy rows");

        remember_ready_account_billing(
            &personal_id,
            AccountBillingFields {
                usage_pct: Some(100.0),
                prepaid_cents: Some(0),
                unified: None,
                period_end: None,
            },
        );
        remember_ready_account_billing(
            &business_id,
            AccountBillingFields {
                usage_pct: Some(20.0),
                prepaid_cents: Some(0),
                unified: None,
                period_end: None,
            },
        );

        DualHeavyHome {
            _home: home,
            _env: env,
            now,
            personal_end,
            business_end,
            personal_id,
            business_id,
        }
    }

    fn dual_heavy_snapshot(personal_end: DateTime<Utc>) -> LimitsSnapshot {
        let mut bal = weekly_bal(100.0, personal_end);
        bal.prepaid_balance_cents = Some(0);
        LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
    }

    fn paint_limits_tab(state: &mut LimitsModalState, tab: usize, now: DateTime<Utc>) -> String {
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 40);
        state.window.active_tab = tab;
        let mut buf = Buffer::empty(area);
        render_limits_modal(&mut buf, area, state, &theme, false, now);
        painted_lines(&buf, area).join("\n")
    }

    fn heavy_block(card: &str, title: &str) -> String {
        let lines: Vec<&str> = card.lines().collect();
        let Some(start) = lines.iter().position(|line| line.contains(title)) else {
            return String::new();
        };
        let end = (start + 3).min(lines.len());
        lines[start..end].join("\n")
    }

    /// Owed: With the Business tab selected, the card contains `Business
    /// SuperGrok Heavy` and does not contain `Personal SuperGrok Heavy`.
    /// Remaining on that row is 80%. SuperGrok dollar credits on that row
    /// are `$0.00` only when that business prepaid reading is known zero.
    /// The reset line names days and hours from the business identity's own
    /// `period_end`. It does not contain the personal remaining 0% Heavy
    /// row. SuperGrok is paid. Meters stay distinct.
    #[test]
    #[serial_test::serial]
    fn business_tab_paints_only_the_business_supergrok_heavy_row() {
        let fixture = seed_dual_heavy_identities();
        let mut state = LimitsModalState::new(dual_heavy_snapshot(fixture.personal_end));
        let card = paint_limits_tab(&mut state, CREDITS_TAB, fixture.now);
        let owed = "with the Business tab selected, the card contains Business SuperGrok Heavy and does not contain Personal SuperGrok Heavy. Remaining on that row is 80%. SuperGrok dollar credits on that row are $0.00 when that business prepaid reading is known zero. The reset line names days and hours from the business identity's own period_end. It does not contain the personal remaining 0% Heavy row.";
        assert!(card.contains("Business SuperGrok Heavy"), "{owed}\n{card}");
        assert!(!card.contains("Personal SuperGrok Heavy"), "{owed}\n{card}");
        let heavy = heavy_block(&card, "Business SuperGrok Heavy");
        assert!(
            heavy.contains("80% remaining") && heavy.contains("$0.00"),
            "{owed}\nheavy block:\n{heavy}\n{card}"
        );
        assert!(card.contains("Resets in: 6d 11h"), "{owed}\n{card}");
        assert!(!card.contains("Resets in: 6d 3h"), "{owed}\n{card}");
    }

    /// Owed: With the Personal tab selected, the card contains `Personal
    /// SuperGrok Heavy` and does not contain `Business SuperGrok Heavy`.
    /// Remaining on that row is 0% when personal used is 100% and that
    /// reading is known. The reset line names days and hours from the
    /// personal identity's own `period_end`. SuperGrok is paid.
    #[test]
    #[serial_test::serial]
    fn personal_tab_paints_only_the_personal_supergrok_heavy_row() {
        let fixture = seed_dual_heavy_identities();
        let mut state = LimitsModalState::new(dual_heavy_snapshot(fixture.personal_end));
        let card = paint_limits_tab(&mut state, LIMITS_TAB, fixture.now);
        let owed = "with the Personal tab selected, the card contains Personal SuperGrok Heavy and does not contain Business SuperGrok Heavy. Remaining on that row is 0% when personal used is 100% and that reading is known. The reset line names days and hours from the personal identity's own period_end.";
        assert!(card.contains("Personal SuperGrok Heavy"), "{owed}\n{card}");
        assert!(!card.contains("Business SuperGrok Heavy"), "{owed}\n{card}");
        let heavy = heavy_block(&card, "Personal SuperGrok Heavy");
        assert!(
            heavy.contains("0% remaining") && heavy.contains("$0.00"),
            "{owed}\nheavy block:\n{heavy}\n{card}"
        );
        assert!(card.contains("Resets in: 6d 3h"), "{owed}\n{card}");
        assert!(!card.contains("Resets in: 6d 11h"), "{owed}\n{card}");
    }

    /// Owed: Personal reset line is `Resets in: 6d 3h` (minutes and seconds
    /// may follow, matching `format_reset_countdown`). Business reset line
    /// is `Resets in: 6d 11h` from a different `next_reset_at`. The two
    /// lines are not the same clock. `earliest_reset_at` is not the
    /// Business tab's painted countdown. Matching day counts are not a
    /// shared pool. SuperGrok is paid. Meters stay distinct.
    #[test]
    #[serial_test::serial]
    fn each_heavy_reset_line_names_days_and_hours_from_that_meters_next_reset() {
        let fixture = seed_dual_heavy_identities();
        let mut state = LimitsModalState::new(dual_heavy_snapshot(fixture.personal_end));
        let personal = paint_limits_tab(&mut state, LIMITS_TAB, fixture.now);
        let business = paint_limits_tab(&mut state, CREDITS_TAB, fixture.now);
        let owed = "the Personal reset line is Resets in: 6d 3h. The Business reset line is Resets in: 6d 11h from a different next_reset_at. The two lines are not the same clock. earliest_reset_at is not the Business tab's painted countdown. Matching day counts are not a shared pool.";
        let personal_countdown = format_reset_countdown(fixture.now, fixture.personal_end);
        let business_countdown = format_reset_countdown(fixture.now, fixture.business_end);
        assert!(
            personal_countdown.starts_with("6d 3h")
                && business_countdown.starts_with("6d 11h")
                && personal_countdown != business_countdown,
            "{owed} formatter personal={personal_countdown} business={business_countdown}"
        );
        assert!(
            personal.contains(&format!("Resets in: {personal_countdown}"))
                && personal.contains("Resets in: 6d 3h"),
            "{owed}\npersonal:\n{personal}"
        );
        assert!(
            business.contains(&format!("Resets in: {business_countdown}"))
                && business.contains("Resets in: 6d 11h"),
            "{owed}\nbusiness:\n{business}"
        );
        assert!(
            !personal.contains(&format!("Resets in: {business_countdown}"))
                && !business.contains(&format!("Resets in: {personal_countdown}")),
            "{owed}\npersonal:\n{personal}\nbusiness:\n{business}"
        );
        let earliest = earliest_reset_at(&state.snapshot).expect("primary next reset");
        assert_eq!(
            earliest, fixture.personal_end,
            "{owed} in-memory snapshot earliest is the personal clock"
        );
        assert_eq!(
            format_reset_countdown(fixture.now, earliest),
            personal_countdown,
            "{owed} earliest_reset_at is the personal countdown"
        );
        assert!(
            !business.contains(&format!(
                "Resets in: {}",
                format_reset_countdown(fixture.now, earliest)
            )),
            "{owed} earliest_reset_at is not the Business tab's painted countdown:\n{business}"
        );
        assert_ne!(
            fixture.personal_end, fixture.business_end,
            "{owed} matching day counts are not a shared pool"
        );
        let doc = xai_grok_shell::auth::read_limits_snapshot_file(&limits_reading_grok_home())
            .expect("dual heavy snapshot");
        let personal_row = doc
            .identities
            .iter()
            .find(|row| row.identity_id == fixture.personal_id)
            .expect("personal identity");
        let business_row = doc
            .identities
            .iter()
            .find(|row| row.identity_id == fixture.business_id)
            .expect("business identity");
        assert_ne!(
            personal_row.period_end, business_row.period_end,
            "{owed} each identity keeps its own nextReset"
        );
    }

    /// When the next request draws business included SuperGrok period
    /// limits, a new Limits card opens on the Business tab. `Using
    /// business limits` is the one copy of that action. `Use business
    /// limits` is not also on the card. Bare `Using credits` is not
    /// painted. SuperGrok is paid. The meters stay distinct.
    #[test]
    #[serial_test::serial]
    fn limits_card_opens_on_business_tab_when_next_request_draws_business_included_limits() {
        use xai_grok_shell::auth::SupergrokIdentityPin;
        use xai_grok_shell::auth::limits_pins::{LimitsPins, MeterSource, save_limits_pins};

        let owed = "when the next request draws business included SuperGrok period limits, the Limits card opens on the Business tab. Using business limits exists once. Use business limits is not also painted. Using credits is not painted.";
        let _seeded = seed_four_cells_for_card_click(None);
        save_limits_pins(&LimitsPins {
            meter_source: Some(MeterSource::Included),
            supergrok_identity: Some(SupergrokIdentityPin::Business),
            use_console: false,
            stay_supergrok: true,
        })
        .expect("business included pin");
        let area = Rect::new(0, 0, 120, 40);
        let mut state = fresh_card_state();
        assert_eq!(
            state.window.active_tab, CREDITS_TAB,
            "{owed} active_tab is Business"
        );
        let buf = render_seeded_card(&mut state, area);
        let tabs = tab_labels(&buf, &state);
        assert_eq!(
            tabs,
            ["Personal", "Business"],
            "{owed} tabs stay Personal then Business"
        );
        let card = painted_lines(&buf, area).join("\n");
        assert!(
            card.contains("Using business limits")
                && card.matches("Using business limits").count() == 1,
            "{owed} Using business limits exists once:\n{card}"
        );
        assert!(
            !card.contains("Use business limits"),
            "{owed} Use business limits is not a second copy:\n{card}"
        );
        assert!(
            !card.contains("Using credits"),
            "{owed} bare Using credits is not painted:\n{card}"
        );
        assert!(
            !card.contains("Using personal limits"),
            "{owed} only the account the next request draws says Using:\n{card}"
        );
        let labels = painted_shortcuts(&state)
            .into_iter()
            .map(|shortcut| shortcut.label)
            .collect::<Vec<_>>();
        let use_personal = labels
            .iter()
            .filter(|label| **label == "Use personal limits")
            .count();
        let use_business = labels
            .iter()
            .filter(|label| **label == "Use business limits")
            .count();
        let using_credits = labels
            .iter()
            .filter(|label| **label == "Using credits")
            .count();
        assert!(
            use_personal <= 1 && use_business == 0 && using_credits == 0,
            "{owed} each action exists at most once. labels={labels:?}"
        );
    }

    /// `Use personal limits`, `Use business limits`, and `Using credits`
    /// each exist at most once on the card. The Personal tab that is not
    /// the next request still offers `Use personal limits` once so the
    /// pin-writer click stays.
    #[test]
    #[serial_test::serial]
    fn use_personal_use_business_and_using_credits_each_exist_once() {
        let owed = "Use personal limits, Use business limits, and Using credits each exist once. They are not painted in the tab body and again in the footer.";
        let _seeded = seed_four_cells_for_card_click(None);
        let area = Rect::new(0, 0, 120, 40);
        let mut state = fresh_card_state();
        assert_eq!(
            state.window.active_tab, LIMITS_TAB,
            "{owed} without a business included pin the card opens on Personal"
        );
        let mut buf = render_seeded_card(&mut state, area);
        let personal = painted_lines(&buf, area).join("\n");
        assert_eq!(
            personal.matches("Use personal limits").count(),
            1,
            "{owed} Use personal limits exists once on Personal:\n{personal}"
        );
        assert!(
            !personal.contains("Use business limits") && !personal.contains("Using credits"),
            "{owed} Personal does not also paint Use business limits or Using credits:\n{personal}"
        );
        assert!(
            select_account_tab(&mut state, &buf, "Business"),
            "{owed} the Business tab is on the card"
        );
        buf = render_seeded_card(&mut state, area);
        let business = painted_lines(&buf, area).join("\n");
        assert_eq!(
            business.matches("Use business limits").count(),
            1,
            "{owed} Use business limits exists once on Business:\n{business}"
        );
        assert!(
            !business.contains("Use personal limits") && !business.contains("Using credits"),
            "{owed} Business does not also paint Use personal limits or Using credits:\n{business}"
        );
    }

    /// A click on each limits button sets that account's included pin and session.
    #[test]
    #[serial_test::serial]
    fn clicking_use_personal_limits_and_use_business_limits_sets_the_pin_and_the_named_session() {
        use xai_grok_shell::auth::SupergrokIdentityPin;

        let seeded = seed_four_cells_for_card_click(None);
        let owed = "A click on `Use personal limits` selects included SuperGrok period limits for the personal session. A click on `Use business limits` selects included SuperGrok period limits for the business session. The test does not call the pin writer.";
        click_exact_footer_phrase("Use personal limits")
            .unwrap_or_else(|err| panic!("owed: {owed} {err}"));
        assert!(
            seeded.home.path().join("limits_pins.json").exists(),
            "owed: {owed}"
        );
        assert_included_session_pin(SupergrokIdentityPin::Personal, CELL_PERSONAL_TOKEN, owed);
        click_exact_footer_phrase("Use business limits")
            .unwrap_or_else(|err| panic!("owed: {owed} {err}"));
        assert_included_session_pin(SupergrokIdentityPin::Business, CELL_TEAM_TOKEN, owed);
        assert_other_three_snapshot_values(seeded.home.path());
    }

    /// `preferred_method = api_key` stays in config. The limits click still
    /// selects that account's SuperGrok session.
    #[test]
    #[serial_test::serial]
    fn preferred_method_api_key_does_not_block_use_personal_or_use_business_limits() {
        use xai_grok_shell::auth::SupergrokIdentityPin;

        let seeded =
            seed_four_cells_for_card_click(Some("[auth]\npreferred_method = \"api_key\"\n"));
        let owed = "preferred_method = api_key does not block `Use personal limits` or `Use business limits`. The click does not switch to the console API key. use_console stays false.";
        let config_text = std::fs::read_to_string(seeded.home.path().join("config.toml"))
            .expect("config.toml stays");
        assert!(
            config_text.contains("preferred_method = \"api_key\""),
            "owed: {owed}"
        );
        click_exact_footer_phrase("Use personal limits")
            .unwrap_or_else(|err| panic!("owed: {owed} {err}"));
        assert_included_session_pin(SupergrokIdentityPin::Personal, CELL_PERSONAL_TOKEN, owed);
        click_exact_footer_phrase("Use business limits")
            .unwrap_or_else(|err| panic!("owed: {owed} {err}"));
        assert_included_session_pin(SupergrokIdentityPin::Business, CELL_TEAM_TOKEN, owed);
        let config_after = std::fs::read_to_string(seeded.home.path().join("config.toml"))
            .expect("config.toml stays");
        assert!(
            config_after.contains("preferred_method = \"api_key\""),
            "owed: {owed}"
        );
        assert_other_three_snapshot_values(seeded.home.path());
    }

    fn write_limits_pin_file(
        home: &std::path::Path,
        pins: &xai_grok_shell::auth::limits_pins::LimitsPins,
    ) {
        std::fs::create_dir_all(home).expect("peer home");
        let bytes = serde_json::to_vec_pretty(pins).expect("prior pin json");
        std::fs::write(home.join("limits_pins.json"), bytes).expect("prior pin file");
    }

    fn write_other_account_auth(home: &std::path::Path) {
        use xai_grok_shell::auth::{AuthMode, GrokAuth, upsert_supergrok_session};

        let live = chrono::Utc::now() + chrono::Duration::days(7);
        let base = "https://auth.x.ai::other-account";
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-other-account-personal".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-other-account".into(),
                principal_type: Some("User".into()),
                principal_id: Some("u-other-account".into()),
                team_id: None,
                expires_at: Some(live),
                ..GrokAuth::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-other-account-team".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-other-account-member".into(),
                principal_type: Some("Team".into()),
                principal_id: Some("team-other-account".into()),
                team_id: Some("team-other-account".into()),
                expires_at: Some(live),
                ..GrokAuth::default()
            },
        );
        std::fs::create_dir_all(home).expect("other account home");
        std::fs::write(
            home.join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("other account auth"),
        )
        .expect("write other account auth");
    }

    /// Nix has `/bin/sh` and coreutils `sleep` on `PATH`. It does not have
    /// `/bin/sleep`. A symlink named `grok-oss-peer` that points at coreutils
    /// `sleep` exits at once, because that multi-call binary reads argv0.
    /// This shell stays up, and the command line still contains `grok`.
    #[cfg(unix)]
    fn peer_shell() -> std::path::PathBuf {
        let candidate = if std::path::Path::new("/bin/sh").is_file() {
            std::path::PathBuf::from("/bin/sh")
        } else {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|dir| dir.join("sh"))
                .find(|path| path.is_file())
                .unwrap_or_else(|| std::path::PathBuf::from("/bin/sh"))
        };
        dunce::canonicalize(&candidate).unwrap_or(candidate)
    }

    /// A live process whose command line contains `grok`. Its grok home is
    /// `home`. This is the other grok-oss process on the L0 list. It does not
    /// implement a limits command. The process group stays alive until this
    /// value is dropped, which is how `ProcessScope` enrolls the OS process.
    struct ListedPeer {
        child: std::process::Child,
        _group: std::sync::Arc<xai_tty_utils::ProcessGroup>,
        home: tempfile::TempDir,
    }

    impl Drop for ListedPeer {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    impl ListedPeer {
        fn spawn(home: tempfile::TempDir) -> Self {
            #[cfg(not(unix))]
            {
                let _ = home;
                panic!(
                    "the L0 list on this host reads a live grok-oss process command line. This test runs where that list can see a process."
                );
            }
            #[cfg(unix)]
            {
                let shell = peer_shell();
                let mut command = std::process::Command::new(&shell);
                command
                    .arg("-c")
                    .arg("while :; do sleep 120; done")
                    .arg("grok-oss-peer")
                    .env("GROK_HOME", home.path())
                    .current_dir(home.path())
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                // `spawn_std` detaches the OS process and calls `enroll_std`.
                let (child, group) = xai_tty_utils::global_process_scope()
                    .spawn_std(command)
                    .expect("peer process");
                Self {
                    child,
                    _group: group,
                    home,
                }
            }
        }
    }

    fn prior_dollar_business_pins() -> xai_grok_shell::auth::limits_pins::LimitsPins {
        use xai_grok_shell::auth::SupergrokIdentityPin;
        use xai_grok_shell::auth::limits_pins::{LimitsPins, MeterSource};
        LimitsPins {
            stay_supergrok: true,
            use_console: false,
            meter_source: Some(MeterSource::DollarCredits),
            supergrok_identity: Some(SupergrokIdentityPin::Business),
        }
    }

    fn pins_are_personal_included(pins: &xai_grok_shell::auth::limits_pins::LimitsPins) -> bool {
        use xai_grok_shell::auth::SupergrokIdentityPin;
        use xai_grok_shell::auth::limits_pins::MeterSource;
        pins.meter_source == Some(MeterSource::Included)
            && pins.supergrok_identity == Some(SupergrokIdentityPin::Personal)
            && !pins.use_console
            && pins.stay_supergrok
    }

    fn pins_stay_prior_dollar_business(
        pins: &xai_grok_shell::auth::limits_pins::LimitsPins,
    ) -> bool {
        use xai_grok_shell::auth::SupergrokIdentityPin;
        use xai_grok_shell::auth::limits_pins::MeterSource;
        pins.meter_source == Some(MeterSource::DollarCredits)
            && pins.supergrok_identity == Some(SupergrokIdentityPin::Business)
            && !pins.use_console
            && pins.stay_supergrok
    }

    /// A click in one grok-oss process applies that choice to the other
    /// process on this host with these accounts. Another host, and a different
    /// account, keep the choice they had. The product has no such command.
    #[test]
    #[serial_test::serial]
    fn a_limits_click_reaches_the_other_grok_oss_process_on_this_host_and_these_accounts() {
        use xai_grok_shell::auth::limits_pins::load_limits_pins_under;

        let seeded = seed_four_cells_for_card_click(None);
        let clicker = seeded.home.path().to_path_buf();
        let same = ListedPeer::spawn(tempfile::TempDir::new().expect("same-account peer"));
        let other_account =
            ListedPeer::spawn(tempfile::TempDir::new().expect("different-account peer"));
        let other_host = tempfile::TempDir::new().expect("other host home");
        std::fs::copy(
            clicker.join("auth.json"),
            same.home.path().join("auth.json"),
        )
        .expect("same accounts");
        std::fs::copy(
            clicker.join("auth.json"),
            other_host.path().join("auth.json"),
        )
        .expect("other host accounts");
        write_other_account_auth(other_account.home.path());
        let prior = prior_dollar_business_pins();
        write_limits_pin_file(same.home.path(), &prior);
        write_limits_pin_file(other_account.home.path(), &prior);
        write_limits_pin_file(other_host.path(), &prior);
        assert_ne!(same.home.path(), clicker.as_path());
        assert_ne!(other_host.path(), clicker.as_path());
        let same_session = xai_grok_active_sessions::ActiveSession::new(
            agent_client_protocol::SessionId::new("peer-same-accounts"),
            same.child.id(),
            same.home.path().display().to_string(),
            Utc::now(),
        );
        let other_account_session = xai_grok_active_sessions::ActiveSession::new(
            agent_client_protocol::SessionId::new("peer-other-account"),
            other_account.child.id(),
            other_account.home.path().display().to_string(),
            Utc::now(),
        );
        xai_grok_active_sessions::register_in(&clicker, same_session)
            .expect("list same-account peer");
        xai_grok_active_sessions::register_in(&clicker, other_account_session)
            .expect("list different-account peer");
        let listed =
            crate::running_sessions::list_running_sessions_in(&clicker).expect("L0 local list");
        assert!(
            listed.iter().any(|row| row.pid == same.child.id()),
            "the same-account peer is a live grok-oss process on this host's L0 list"
        );
        assert!(
            listed.iter().any(|row| row.pid == other_account.child.id()),
            "the different-account peer is a live grok-oss process on this host's L0 list"
        );
        assert!(
            pins_stay_prior_dollar_business(&load_limits_pins_under(same.home.path())),
            "the peer starts on its own SuperGrok dollar credits pin"
        );
        let click = click_exact_footer_phrase("Use personal limits");
        let clicker_pins = xai_grok_shell::auth::limits_pins::load_limits_pins();
        let peer_pins = load_limits_pins_under(same.home.path());
        let other_host_pins = load_limits_pins_under(other_host.path());
        let other_account_pins = load_limits_pins_under(other_account.home.path());
        let click_ok = click.is_ok() && pins_are_personal_included(&clicker_pins);
        let peer_ok = pins_are_personal_included(&peer_pins);
        let others_ok = pins_stay_prior_dollar_business(&other_host_pins)
            && pins_stay_prior_dollar_business(&other_account_pins);
        assert!(
            click_ok && peer_ok && others_ok,
            "a click on `Use personal limits` in one grok-oss process writes that choice and applies it to the other grok-oss process on this host with these accounts. The peer keeps its own limits_pins.json at {}. Sharing the clicker's file is not that delivery. A process on another host, and a process on a different account, keep SuperGrok dollar credits for the business session. The product has no such command. click={click:?} clicker={clicker_pins:?} peer={peer_pins:?} other_host={other_host_pins:?} other_account={other_account_pins:?}",
            same.home.path().display()
        );
    }

    struct FetchedAccountBilling {
        identity_id: String,
        role: &'static str,
        usage_pct: Option<f64>,
        prepaid_cents: Option<i64>,
        unified: Option<bool>,
    }

    fn logged_in_account_home() -> Result<std::path::PathBuf, String> {
        std::env::var_os("GROK_HOME")
            .filter(|value| !value.is_empty())
            .map(std::path::PathBuf::from)
            .ok_or_else(|| "GROK_HOME is unset. The card reads that temp home.".to_string())
    }

    fn role_for_identity(
        identity_id: &str,
        listings: &[xai_grok_shell::auth::SupergrokPrincipalListing],
        candidates: &[xai_grok_shell::auth::SupergrokSessionCandidate],
    ) -> Option<&'static str> {
        if let Some(listing) = listings
            .iter()
            .find(|listing| listing.identity_id == identity_id)
        {
            return Some(listing.role_label);
        }
        candidates
            .iter()
            .find(|candidate| candidate.headroom.identity_id == identity_id)
            .map(|candidate| xai_grok_shell::auth::role_label(candidate.headroom.role))
    }

    async fn fetch_logged_in_account(
        target: &xai_grok_shell::auth::SupergrokBillingPollTarget,
        role: &'static str,
    ) -> Result<FetchedAccountBilling, String> {
        let base = account_billing_proxy_base();
        let url = format!("{}/billing?format=credits", base.trim_end_matches('/'));
        let fetched = xai_grok_shell::extensions::billing::fetch_credits_config_with_session(
            &base,
            &target.access_token,
            &target.user_id,
        )
        .await
        .map_err(|err| {
            let shown = err.replace(&target.access_token, "[session]");
            format!(
                "GET {url} for the {role} session {} failed: {shown}",
                target.identity_id
            )
        })?;
        let Some(config) = fetched.config else {
            return Err(format!(
                "GET {url} for the {role} session {} returned no billing config",
                target.identity_id
            ));
        };
        Ok(FetchedAccountBilling {
            identity_id: target.identity_id.clone(),
            role,
            usage_pct: config.credit_usage_percent,
            prepaid_cents: config.prepaid_balance.map(|cent| cent.val),
            unified: config.is_unified_billing_user,
        })
    }

    fn figure_matches_response(
        section: &str,
        usage_pct: Option<f64>,
        prepaid_cents: Option<i64>,
        kind: &str,
    ) -> Option<String> {
        if kind == "Limits" {
            match limits_percent_figure(usage_pct) {
                Some(figure) => {
                    if section.contains(&figure) {
                        None
                    } else {
                        Some(format!(
                            "the Limits section shows that response's included usage percent `{figure}`"
                        ))
                    }
                }
                None => {
                    if section.contains("limits 0%") || section.contains("limits ") {
                        Some(
                            "an empty included usage percent stays empty. It does not become `limits 0%`."
                                .to_string(),
                        )
                    } else {
                        None
                    }
                }
            }
        } else {
            match prepaid_cents.map(format_remaining_cents_as_dollars) {
                Some(figure) => {
                    if section.contains(&figure) {
                        None
                    } else {
                        Some(format!(
                            "the Credits section shows that response's prepaid balance `{figure}`"
                        ))
                    }
                }
                None => {
                    if section.contains('$') || section.contains("$0") {
                        Some(
                            "an empty prepaid balance stays empty. It does not become `$0`."
                                .to_string(),
                        )
                    } else {
                        None
                    }
                }
            }
        }
    }

    fn unified_flag_contradicts(text: &str, flag: Option<bool>) -> bool {
        let lower = text.to_ascii_lowercase();
        match flag {
            None => {
                lower.contains("is_unified_billing_user: true")
                    || lower.contains("is_unified_billing_user: false")
                    || lower.contains("unified billing user: true")
                    || lower.contains("unified billing user: false")
            }
            Some(true) => {
                lower.contains("is_unified_billing_user: false")
                    || lower.contains("unified billing user: false")
            }
            Some(false) => {
                lower.contains("is_unified_billing_user: true")
                    || lower.contains("unified billing user: true")
            }
        }
    }

    const PERSONAL_BILLING_TOKEN: &str = "fixture.personal.session";
    const BUSINESS_BILLING_TOKEN: &str = "fixture.business.session";
    const PERSONAL_BILLING_USER: &str = "fixture-personal-user";
    const BUSINESS_BILLING_USER: &str = "fixture-business-user";
    const BUSINESS_BILLING_TEAM: &str = "fixture-business-team";
    /// 37 percent used is `limits 63%`. Not 10 percent used.
    const PERSONAL_USAGE_PCT: f64 = 37.0;
    /// 54 percent used is `limits 46%`. Not 10 percent used.
    const BUSINESS_USAGE_PCT: f64 = 54.0;
    /// `$64.21`. Not `$78.41`.
    const PERSONAL_PREPAID_CENTS: i64 = 6_421;
    /// `$18.08`. Not `$78.41`.
    const BUSINESS_PREPAID_CENTS: i64 = 1_808;
    const PERSONAL_BILLING_BODY: &str = concat!(
        r#"{"config":{"creditUsagePercent":37.0,"prepaidBalance":{"val":6421},"#,
        r#""isUnifiedBillingUser":true}}"#
    );
    const BUSINESS_BILLING_BODY: &str = concat!(
        r#"{"config":{"creditUsagePercent":54.0,"prepaidBalance":{"val":1808},"#,
        r#""isUnifiedBillingUser":false}}"#
    );

    struct AccountBillingProxyGuard;

    impl AccountBillingProxyGuard {
        fn arm(base: &str) -> Self {
            set_account_billing_proxy_override(Some(base.to_string()));
            Self
        }
    }

    impl Drop for AccountBillingProxyGuard {
        fn drop(&mut self) {
            set_account_billing_proxy_override(None);
            clear_account_billing_cache();
        }
    }

    struct AccountBillingFixture {
        base: String,
        hits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl AccountBillingFixture {
        fn start() -> Self {
            use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("billing fixture listen");
            let port = listener.local_addr().expect("billing fixture port").port();
            listener
                .set_nonblocking(true)
                .expect("billing fixture nonblocking");
            let hits = std::sync::Arc::new(AtomicUsize::new(0));
            let stop = std::sync::Arc::new(AtomicBool::new(false));
            let hits_bg = std::sync::Arc::clone(&hits);
            let stop_bg = std::sync::Arc::clone(&stop);
            let thread = std::thread::Builder::new()
                .name("account-billing-fixture".to_owned())
                .spawn(move || {
                    while !stop_bg.load(Ordering::SeqCst) {
                        match listener.accept() {
                            Ok((mut stream, _)) => {
                                hits_bg.fetch_add(1, Ordering::SeqCst);
                                let request = read_http_headers(&mut stream);
                                let body = if request.contains(PERSONAL_BILLING_TOKEN) {
                                    PERSONAL_BILLING_BODY
                                } else if request.contains(BUSINESS_BILLING_TOKEN) {
                                    BUSINESS_BILLING_BODY
                                } else {
                                    let _ = write_billing_http_response(
                                        &mut stream,
                                        401,
                                        r#"{"error":"no billing fixture"}"#,
                                    );
                                    continue;
                                };
                                let _ = write_billing_http_response(&mut stream, 200, body);
                            }
                            Err(err)
                                if err.kind() == std::io::ErrorKind::WouldBlock
                                    || err.kind() == std::io::ErrorKind::Interrupted =>
                            {
                                std::thread::sleep(std::time::Duration::from_millis(5));
                            }
                            Err(_) => break,
                        }
                    }
                })
                .expect("billing fixture thread");
            Self {
                base: format!("http://127.0.0.1:{port}"),
                hits,
                stop,
                thread: Some(thread),
            }
        }

        fn hits(&self) -> usize {
            self.hits.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl Drop for AccountBillingFixture {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn read_http_headers(stream: &mut std::net::TcpStream) -> String {
        use std::io::Read;

        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        while !buf.windows(4).any(|window| window == b"\r\n\r\n") && buf.len() < 8192 {
            match stream.read(&mut tmp) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }

    fn write_billing_http_response(
        stream: &mut std::net::TcpStream,
        status: u16,
        body: &str,
    ) -> std::io::Result<()> {
        use std::io::Write;

        let reason = if status == 200 { "OK" } else { "Unauthorized" };
        let header = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(body.as_bytes())?;
        stream.flush()
    }

    fn write_personal_and_business_sessions(home: &std::path::Path) {
        use xai_grok_shell::auth::{AuthMode, GrokAuth, upsert_supergrok_session};

        let live = Utc::now() + chrono::Duration::days(7);
        let base = "https://auth.x.ai::account-billing-fixture";
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: PERSONAL_BILLING_TOKEN.into(),
                auth_mode: AuthMode::Oidc,
                user_id: PERSONAL_BILLING_USER.into(),
                principal_type: Some("User".into()),
                principal_id: Some(PERSONAL_BILLING_USER.into()),
                team_id: None,
                expires_at: Some(live),
                ..GrokAuth::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: BUSINESS_BILLING_TOKEN.into(),
                auth_mode: AuthMode::Oidc,
                user_id: BUSINESS_BILLING_USER.into(),
                principal_type: Some("Team".into()),
                principal_id: Some(BUSINESS_BILLING_TEAM.into()),
                team_id: Some(BUSINESS_BILLING_TEAM.into()),
                expires_at: Some(live),
                ..GrokAuth::default()
            },
        );
        std::fs::write(
            home.join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth json"),
        )
        .expect("write auth.json");
    }

    fn collapsed_line(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn assert_empty_discovery_paints_no_billing_success(server: &AccountBillingFixture) {
        let area = Rect::new(0, 0, 120, 40);
        let mut state = fresh_card_state();
        let mut buf = render_seeded_card(&mut state, area);
        let personal = account_body_lines(&buf, area, &state).join("\n");
        assert!(
            collapsed_line(&personal)
                .contains("Personal SuperGrok has no session on this process."),
            "an empty discovery names the missing personal session and does not paint a billing success:\n{personal}"
        );
        assert!(
            select_account_tab(&mut state, &buf, "Business"),
            "the Business tab stays on the card when no session is stored"
        );
        buf = render_seeded_card(&mut state, area);
        let business = account_body_lines(&buf, area, &state).join("\n");
        assert!(
            collapsed_line(&business)
                .contains("Business SuperGrok has no session on this process."),
            "an empty discovery names the missing business session and does not paint a billing success:\n{business}"
        );
        for body in [&personal, &business] {
            assert!(
                !body.contains("Prepaid balance:"),
                "an empty discovery does not paint a prepaid balance:\n{body}"
            );
            assert!(
                !body.contains("is_unified_billing_user:"),
                "an empty discovery does not paint a billing flag:\n{body}"
            );
            for absent in [
                "limits 63%",
                "limits 46%",
                "$64.21",
                "$18.08",
                "$78.41",
                "limits 90%",
                "Console API credits",
                "Billing Credits",
            ] {
                assert!(
                    !body.contains(absent),
                    "an empty discovery does not paint `{absent}`:\n{body}"
                );
            }
        }
        assert_eq!(
            server.hits(),
            0,
            "an empty discovery does not call the billing HTTP endpoint"
        );
    }

    fn push_owned_section(
        misses: &mut Vec<String>,
        lines: &[String],
        kind: &str,
        tab: &str,
        own: &str,
        other: &str,
    ) {
        let Some(section) = section_text(lines, kind) else {
            misses.push(format!(
                "the {tab} tab's {kind} section shows that account's billing response."
            ));
            return;
        };
        if !section.contains(own) {
            misses.push(format!(
                "the {tab} {kind} section shows `{own}` from that account's billing response."
            ));
        }
        if section.contains(other) {
            misses.push(format!(
                "the {tab} {kind} section shows the other account's `{other}`."
            ));
        }
        for absent in [
            "$78.41",
            "limits 90%",
            "Console API credits",
            "Billing Credits",
            "no session on this process",
        ] {
            if section.contains(absent) {
                misses.push(format!(
                    "the {tab} {kind} section does not show `{absent}`."
                ));
            }
        }
    }

    /// Personal and Business sections show each logged-in account's own
    /// `GET /billing?format=credits` body. The temp home stores one personal
    /// session and one business session. Empty discovery does not panic and
    /// does not paint either body.
    #[tokio::test]
    #[serial_test::serial]
    async fn personal_and_business_sections_show_each_logged_in_accounts_billing_response() {
        let temp = tempfile::TempDir::new().expect("temp grok home");
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", temp.path());
        let _cooldown = xai_grok_test_support::EnvGuard::set("GROK_DISABLE_SHARED_RATE_LIMIT", "1");
        clear_account_billing_cache();
        let server = AccountBillingFixture::start();
        let _proxy = AccountBillingProxyGuard::arm(&server.base);
        assert_empty_discovery_paints_no_billing_success(&server);
        write_personal_and_business_sessions(temp.path());
        clear_account_billing_cache();
        let home = logged_in_account_home().unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(
            home.as_path(),
            temp.path(),
            "discovery reads the temp home that stores the two sessions"
        );
        assert_eq!(
            limits_reading_grok_home(),
            temp.path(),
            "the open card reads the same temp home as discovery"
        );
        let targets = xai_grok_shell::auth::load_supergrok_billing_poll_targets(&home);
        let listings = xai_grok_shell::auth::read_auth_json(&home.join("auth.json"))
            .map(|map| xai_grok_shell::auth::list_supergrok_principal_listings(&map))
            .unwrap_or_default();
        let candidates = xai_grok_shell::auth::load_supergrok_session_candidates(&home);
        if targets.is_empty() {
            panic!(
                "account discovery found no logged-in SuperGrok session under {}. The temp home stores a personal session and a business session.",
                home.display()
            );
        }
        let mut personal: Option<FetchedAccountBilling> = None;
        let mut business: Option<FetchedAccountBilling> = None;
        let mut misses = Vec::new();
        for target in &targets {
            let Some(role) = role_for_identity(&target.identity_id, &listings, &candidates) else {
                misses.push(format!(
                    "poll target {} has no personal or business role in the logged-in account list",
                    target.identity_id
                ));
                continue;
            };
            if role != "personal" && role != "business" {
                continue;
            }
            match fetch_logged_in_account(target, role).await {
                Ok(fetched) => match role {
                    "personal" => personal = Some(fetched),
                    "business" => business = Some(fetched),
                    _ => {}
                },
                Err(err) => misses.push(err),
            }
        }
        let Some(personal) = personal else {
            panic!(
                "discovery did not accept the personal session under {}.\n{}",
                home.display(),
                misses.join("\n")
            );
        };
        let Some(business) = business else {
            panic!(
                "discovery did not accept the business session under {}.\n{}",
                home.display(),
                misses.join("\n")
            );
        };
        assert_eq!(personal.usage_pct, Some(PERSONAL_USAGE_PCT));
        assert_eq!(business.usage_pct, Some(BUSINESS_USAGE_PCT));
        assert_eq!(personal.prepaid_cents, Some(PERSONAL_PREPAID_CENTS));
        assert_eq!(business.prepaid_cents, Some(BUSINESS_PREPAID_CENTS));
        assert_eq!(personal.unified, Some(true));
        assert_eq!(business.unified, Some(false));
        assert_ne!(personal.usage_pct, business.usage_pct);
        assert_ne!(personal.prepaid_cents, business.prepaid_cents);
        assert!(
            server.hits() >= 2,
            "the personal session and the business session each received their own billing HTTP response. hits={}",
            server.hits()
        );
        let area = Rect::new(0, 0, 120, 40);
        let mut state = fresh_card_state();
        for account in [&personal, &business] {
            let mut buf = render_seeded_card(&mut state, area);
            if !select_account_tab(&mut state, &buf, account_tab_title(account.role)) {
                misses.push(format!(
                    "the {} tab is not on the card. GET /billing?format=credits for {} returned included usage {:?} and prepaid cents {:?}. is_unified_billing_user is {:?}.",
                    account_tab_title(account.role),
                    account.identity_id,
                    account.usage_pct,
                    account.prepaid_cents,
                    account.unified
                ));
                continue;
            }
            buf = render_seeded_card(&mut state, area);
            let body = account_body_lines(&buf, area, &state);
            let body_text = body.join("\n");
            if unified_flag_contradicts(&body_text, account.unified) {
                misses.push(format!(
                    "is_unified_billing_user for {} stays {:?}.",
                    account.identity_id, account.unified
                ));
            }
            for kind in ["Limits", "Credits"] {
                let Some(section) = section_text(&body, kind) else {
                    misses.push(format!(
                        "the {} tab's {kind} section shows that account's billing response.",
                        account_tab_title(account.role)
                    ));
                    continue;
                };
                if let Some(miss) = figure_matches_response(
                    &section,
                    account.usage_pct,
                    account.prepaid_cents,
                    kind,
                ) {
                    misses.push(format!(
                        "{} session {}: {miss}",
                        account.role, account.identity_id
                    ));
                }
            }
        }
        if let (Some(personal_limits), Some(business_limits)) = (
            limits_percent_figure(personal.usage_pct),
            limits_percent_figure(business.usage_pct),
        ) {
            if personal_limits != business_limits {
                let buf = render_seeded_card(&mut state, area);
                let card = painted_lines(&buf, area).join("\n");
                if card.matches(&personal_limits).count() > 0
                    && card.contains(&business_limits)
                    && tab_labels(&buf, &state) == ["Personal", "Business"]
                {
                    let mut personal_state = fresh_card_state();
                    let mut personal_buf = render_seeded_card(&mut personal_state, area);
                    if select_account_tab(&mut personal_state, &personal_buf, "Personal") {
                        personal_buf = render_seeded_card(&mut personal_state, area);
                        let personal_body =
                            account_body_lines(&personal_buf, area, &personal_state);
                        if let Some(section) = section_text(&personal_body, "Limits") {
                            if section.contains(&business_limits) {
                                misses.push(
                                    "the Personal Limits section shows the business account's included usage percent."
                                        .to_string(),
                                );
                            }
                        }
                    }
                }
            }
        }
        if personal.prepaid_cents != business.prepaid_cents {
            if let (Some(personal_dollars), Some(business_dollars)) = (
                personal
                    .prepaid_cents
                    .map(format_remaining_cents_as_dollars),
                business
                    .prepaid_cents
                    .map(format_remaining_cents_as_dollars),
            ) {
                if personal_dollars != business_dollars {
                    let mut state = fresh_card_state();
                    let mut buf = render_seeded_card(&mut state, area);
                    if select_account_tab(&mut state, &buf, "Business") {
                        buf = render_seeded_card(&mut state, area);
                        let body = account_body_lines(&buf, area, &state);
                        if let Some(section) = section_text(&body, "Credits") {
                            if section.contains(&personal_dollars) {
                                misses.push(
                                    "the Business Credits section shows the personal account's prepaid balance."
                                        .to_string(),
                                );
                            }
                        }
                    }
                }
            }
        }
        let personal_limits =
            limits_percent_figure(personal.usage_pct).expect("personal included percent");
        let business_limits =
            limits_percent_figure(business.usage_pct).expect("business included percent");
        let personal_dollars = format_remaining_cents_as_dollars(
            personal.prepaid_cents.expect("personal prepaid cents"),
        );
        let business_dollars = format_remaining_cents_as_dollars(
            business.prepaid_cents.expect("business prepaid cents"),
        );
        assert_ne!(personal_limits, business_limits);
        assert_ne!(personal_dollars, business_dollars);
        let mut personal_state = fresh_card_state();
        let mut personal_buf = render_seeded_card(&mut personal_state, area);
        assert!(
            select_account_tab(&mut personal_state, &personal_buf, "Personal"),
            "the Personal tab is on the card"
        );
        personal_buf = render_seeded_card(&mut personal_state, area);
        let personal_body = account_body_lines(&personal_buf, area, &personal_state);
        push_owned_section(
            &mut misses,
            &personal_body,
            "Limits",
            "Personal",
            &personal_limits,
            &business_limits,
        );
        push_owned_section(
            &mut misses,
            &personal_body,
            "Credits",
            "Personal",
            &personal_dollars,
            &business_dollars,
        );
        let mut business_state = fresh_card_state();
        let mut business_buf = render_seeded_card(&mut business_state, area);
        assert!(
            select_account_tab(&mut business_state, &business_buf, "Business"),
            "the Business tab is on the card"
        );
        business_buf = render_seeded_card(&mut business_state, area);
        let business_body = account_body_lines(&business_buf, area, &business_state);
        push_owned_section(
            &mut misses,
            &business_body,
            "Limits",
            "Business",
            &business_limits,
            &personal_limits,
        );
        push_owned_section(
            &mut misses,
            &business_body,
            "Credits",
            "Business",
            &business_dollars,
            &personal_dollars,
        );
        assert!(
            misses.is_empty(),
            "the card shows each logged-in account's own GET /billing?format=credits body. One account's percent or prepaid balance is not copied onto the other account.\n{}",
            misses.join("\n")
        );
    }

    fn account_tab_title(role: &str) -> &'static str {
        if role == "business" {
            "Business"
        } else {
            "Personal"
        }
    }

    struct CardRefreshListener {
        port: u16,
        token_hits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl CardRefreshListener {
        fn start() -> Self {
            use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("refresh listen");
            let port = listener.local_addr().expect("refresh port").port();
            listener.set_nonblocking(true).expect("refresh nonblocking");
            let token_hits = std::sync::Arc::new(AtomicUsize::new(0));
            let stop = std::sync::Arc::new(AtomicBool::new(false));
            let hits_bg = std::sync::Arc::clone(&token_hits);
            let stop_bg = std::sync::Arc::clone(&stop);
            let thread = std::thread::Builder::new()
                .name("card-refresh-listener".to_owned())
                .spawn(move || {
                    let discovery = format!(
                        r#"{{"authorization_endpoint":"http://127.0.0.1:{port}/authorize","token_endpoint":"http://127.0.0.1:{port}/token"}}"#
                    );
                    while !stop_bg.load(Ordering::SeqCst) {
                        match listener.accept() {
                            Ok((mut stream, _)) => {
                                let _ = stream.set_nonblocking(false);
                                let request = read_http_headers(&mut stream);
                                let (status, body, count_token) = if request
                                    .contains("/.well-known/openid-configuration")
                                {
                                    (200, discovery.clone(), false)
                                } else if request.contains("/token") {
                                    (
                                        200,
                                        r#"{"access_token":"new.personal.session","expires_in":3600}"#
                                            .to_owned(),
                                        true,
                                    )
                                } else if request.contains("/billing")
                                    && request.contains("new.personal.session")
                                {
                                    (
                                        200,
                                        r#"{"config":{"creditUsagePercent":37.0,"prepaidBalance":{"val":1234}}}"#
                                            .to_owned(),
                                        false,
                                    )
                                } else if request.contains("expired.personal.jwt") {
                                    (401, r#"{"error":"expired"}"#.to_owned(), false)
                                } else {
                                    (401, r#"{"error":"unexpected"}"#.to_owned(), false)
                                };
                                if count_token {
                                    hits_bg.fetch_add(1, Ordering::SeqCst);
                                }
                                let _ = write_billing_http_response(&mut stream, status, &body);
                            }
                            Err(err)
                                if err.kind() == std::io::ErrorKind::WouldBlock
                                    || err.kind() == std::io::ErrorKind::Interrupted =>
                            {
                                std::thread::sleep(std::time::Duration::from_millis(5));
                            }
                            Err(_) => break,
                        }
                    }
                })
                .expect("refresh listener thread");
            Self {
                port,
                token_hits,
                stop,
                thread: Some(thread),
            }
        }

        fn token_hits(&self) -> usize {
            self.token_hits.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl Drop for CardRefreshListener {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// A hard-expired personal OIDC token is refreshed before the card GET.
    /// The new session is the one this process uses. 37 percent used paints
    /// `limits 63%`. The snapshot row for that identity is poll outcome ok.
    #[test]
    #[serial_test::serial]
    fn expired_personal_refresh_becomes_the_session_this_process_uses() {
        use xai_grok_shell::auth::{AuthMode, GrokAuth, upsert_supergrok_session};

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let _cooldown = xai_grok_test_support::EnvGuard::set("GROK_DISABLE_SHARED_RATE_LIMIT", "1");
        clear_account_billing_cache();
        xai_grok_shell::auth::clear_included_billing_cache();
        let server = CardRefreshListener::start();
        let _proxy = AccountBillingProxyGuard::arm(&format!("http://127.0.0.1:{}/v1", server.port));
        let issuer = format!("http://127.0.0.1:{}", server.port);
        let base = format!("{issuer}::refresh-card");
        let expired = chrono::Utc::now() - chrono::Duration::days(1);
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            &base,
            GrokAuth {
                key: "expired.personal.jwt".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-refresh-personal".into(),
                principal_type: Some("User".into()),
                principal_id: Some("u-refresh-personal".into()),
                team_id: None,
                expires_at: Some(expired),
                refresh_token: Some("refresh-synthetic-personal".into()),
                oidc_issuer: Some(issuer),
                oidc_client_id: Some("client-synthetic-personal".into()),
                ..GrokAuth::default()
            },
        );
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth json"),
        )
        .expect("write auth.json");

        let body = account_section_lines(
            xai_grok_shell::auth::SupergrokAccountRole::Personal,
            Utc::now(),
        )
        .join("\n");
        assert!(
            server.token_hits() >= 1,
            "the expired personal session refreshed before the card GET"
        );
        let stored = xai_grok_shell::auth::read_auth_json(&home.path().join("auth.json"))
            .expect("auth.json");
        let personal_key = stored
            .iter()
            .find(|(scope, _)| scope.contains("::personal"))
            .map(|(_, auth)| auth.key.as_str())
            .expect("personal slot");
        assert_eq!(personal_key, "new.personal.session");
        assert!(
            !body.contains("Personal SuperGrok has no session on this process."),
            "a refreshed personal session is the session this process uses:\n{body}"
        );
        assert!(
            body.contains("limits 63%") && !body.contains("limits 37%"),
            "37 percent used is limits 63% remaining:\n{body}"
        );
        assert!(
            !body.contains("$78.41"),
            "this personal prepaid balance is not $78.41:\n{body}"
        );
        let doc =
            xai_grok_shell::auth::read_limits_snapshot_file(home.path()).expect("limits snapshot");
        let row = doc
            .identities
            .iter()
            .find(|row| row.identity_id == "u-refresh-personal")
            .expect("refreshed identity row");
        assert_eq!(row.poll_outcome, xai_grok_shell::auth::POLL_OUTCOME_OK);
        assert_eq!(row.usage_pct, Some(37.0));
        assert_ne!(row.poll_outcome, "auth");
    }

    struct RejectedPersonalRefreshListener {
        port: u16,
        billing_authorizations: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        billing_hits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl RejectedPersonalRefreshListener {
        fn start() -> Self {
            use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("rejected listen");
            let port = listener.local_addr().expect("rejected port").port();
            listener
                .set_nonblocking(true)
                .expect("rejected nonblocking");
            let billing_authorizations = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let billing_hits = std::sync::Arc::new(AtomicUsize::new(0));
            let stop = std::sync::Arc::new(AtomicBool::new(false));
            let auths_bg = std::sync::Arc::clone(&billing_authorizations);
            let hits_bg = std::sync::Arc::clone(&billing_hits);
            let stop_bg = std::sync::Arc::clone(&stop);
            let thread = std::thread::Builder::new()
                .name("rejected-personal-refresh".to_owned())
                .spawn(move || {
                    let discovery = format!(
                        r#"{{"authorization_endpoint":"http://127.0.0.1:{port}/authorize","token_endpoint":"http://127.0.0.1:{port}/token"}}"#
                    );
                    while !stop_bg.load(Ordering::SeqCst) {
                        match listener.accept() {
                            Ok((mut stream, _)) => {
                                let _ = stream.set_nonblocking(false);
                                let request = read_http_headers(&mut stream);
                                let (status, body) = if request
                                    .contains("/.well-known/openid-configuration")
                                {
                                    (200, discovery.clone())
                                } else if request.contains("/token") {
                                    (
                                        400,
                                        r#"{"error":"invalid_grant","error_description":"refresh token revoked"}"#
                                            .to_owned(),
                                    )
                                } else if request.contains("/billing") {
                                    hits_bg.fetch_add(1, Ordering::SeqCst);
                                    let authorization = request
                                        .lines()
                                        .find(|line| {
                                            line.to_ascii_lowercase().starts_with("authorization:")
                                        })
                                        .unwrap_or("")
                                        .to_owned();
                                    if let Ok(mut guard) = auths_bg.lock() {
                                        guard.push(authorization);
                                    }
                                    (401, r#"{"error":"refused"}"#.to_owned())
                                } else {
                                    (404, r#"{"error":"unexpected"}"#.to_owned())
                                };
                                let _ = write_billing_http_response(&mut stream, status, &body);
                            }
                            Err(err)
                                if err.kind() == std::io::ErrorKind::WouldBlock
                                    || err.kind() == std::io::ErrorKind::Interrupted =>
                            {
                                std::thread::sleep(std::time::Duration::from_millis(5));
                            }
                            Err(_) => break,
                        }
                    }
                })
                .expect("rejected refresh listener thread");
            Self {
                port,
                billing_authorizations,
                billing_hits,
                stop,
                thread: Some(thread),
            }
        }

        fn billing_hits(&self) -> usize {
            self.billing_hits.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn billing_authorizations(&self) -> Vec<String> {
            self.billing_authorizations
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default()
        }
    }

    impl Drop for RejectedPersonalRefreshListener {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// Owed outcome: a None personal refresh does not place the stored
    /// personal access token on the request and does not place the business
    /// token on the request. That credits request is skipped.
    #[test]
    #[serial_test::serial]
    fn none_personal_refresh_does_not_place_the_stored_personal_access_token_or_the_business_token_on_the_request()
     {
        use xai_grok_shell::auth::{AuthMode, GrokAuth, upsert_supergrok_session};

        const STORED_PERSONAL: &str = "expired.personal.access";
        const BUSINESS_TOKEN: &str = "live.business.session";

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let _cooldown = xai_grok_test_support::EnvGuard::set("GROK_DISABLE_SHARED_RATE_LIMIT", "1");
        clear_account_billing_cache();
        xai_grok_shell::auth::clear_included_billing_cache();
        let server = RejectedPersonalRefreshListener::start();
        let _proxy = AccountBillingProxyGuard::arm(&format!("http://127.0.0.1:{}/v1", server.port));
        let issuer = format!("http://127.0.0.1:{}", server.port);
        let expired = chrono::Utc::now() - chrono::Duration::days(1);
        let live = chrono::Utc::now() + chrono::Duration::days(7);
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            &format!("{issuer}::none-personal"),
            GrokAuth {
                key: STORED_PERSONAL.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-none-personal".into(),
                principal_type: Some("User".into()),
                principal_id: Some("u-none-personal".into()),
                team_id: None,
                expires_at: Some(expired),
                refresh_token: Some("refresh-none-personal".into()),
                oidc_issuer: Some(issuer),
                oidc_client_id: Some("client-none-personal".into()),
                ..GrokAuth::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            "https://auth.x.ai::none-business",
            GrokAuth {
                key: BUSINESS_TOKEN.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-none-business".into(),
                principal_type: Some("Team".into()),
                principal_id: Some("team-none-business".into()),
                team_id: Some("team-none-business".into()),
                expires_at: Some(live),
                ..GrokAuth::default()
            },
        );
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth json"),
        )
        .expect("write auth.json");

        let _body = account_section_lines(
            xai_grok_shell::auth::SupergrokAccountRole::Personal,
            Utc::now(),
        )
        .join("\n");
        let authorizations = server.billing_authorizations();
        let hits = server.billing_hits();
        assert!(
            hits == 0
                && authorizations.iter().all(|header| {
                    !header.contains(STORED_PERSONAL) && !header.contains(BUSINESS_TOKEN)
                }),
            "a None personal refresh does not place the stored personal access token `{STORED_PERSONAL}` or the business token `{BUSINESS_TOKEN}` on the request; billing hits were {hits} and authorizations were {authorizations:?}"
        );
    }

    struct LivePersonalBearerListener {
        base: String,
        saw_live: std::sync::Arc<std::sync::atomic::AtomicBool>,
        saw_expired: std::sync::Arc<std::sync::atomic::AtomicBool>,
        hits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl LivePersonalBearerListener {
        const LIVE_TOKEN: &'static str = "live.personal.login";
        const EXPIRED_TOKEN: &'static str = "expired.personal.row";

        fn start() -> Self {
            use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("credits listen");
            let port = listener.local_addr().expect("credits port").port();
            listener.set_nonblocking(true).expect("credits nonblocking");
            let saw_live = std::sync::Arc::new(AtomicBool::new(false));
            let saw_expired = std::sync::Arc::new(AtomicBool::new(false));
            let hits = std::sync::Arc::new(AtomicUsize::new(0));
            let stop = std::sync::Arc::new(AtomicBool::new(false));
            let live_bg = std::sync::Arc::clone(&saw_live);
            let expired_bg = std::sync::Arc::clone(&saw_expired);
            let hits_bg = std::sync::Arc::clone(&hits);
            let stop_bg = std::sync::Arc::clone(&stop);
            let thread = std::thread::Builder::new()
                .name("live-personal-bearer".to_owned())
                .spawn(move || {
                    let live_body = r#"{"config":{"creditUsagePercent":10.0}}"#;
                    let expired_body =
                        r#"{"config":{"creditUsagePercent":100.0,"prepaidBalance":{"val":7841}}}"#;
                    while !stop_bg.load(Ordering::SeqCst) {
                        match listener.accept() {
                            Ok((mut stream, _)) => {
                                let _ = stream.set_nonblocking(false);
                                let request = read_http_headers(&mut stream);
                                if !request.contains("/billing") {
                                    let _ = write_billing_http_response(
                                        &mut stream,
                                        404,
                                        r#"{"error":"not billing"}"#,
                                    );
                                    continue;
                                }
                                hits_bg.fetch_add(1, Ordering::SeqCst);
                                let authorization = request.lines().find(|line| {
                                    line.to_ascii_lowercase().starts_with("authorization:")
                                });
                                let (status, body) = match authorization {
                                    Some(line) if line.contains(Self::LIVE_TOKEN) => {
                                        live_bg.store(true, Ordering::SeqCst);
                                        (200, live_body)
                                    }
                                    Some(line) if line.contains(Self::EXPIRED_TOKEN) => {
                                        expired_bg.store(true, Ordering::SeqCst);
                                        (200, expired_body)
                                    }
                                    _ => (401, r#"{"error":"unexpected bearer"}"#),
                                };
                                let _ = write_billing_http_response(&mut stream, status, body);
                            }
                            Err(err)
                                if err.kind() == std::io::ErrorKind::WouldBlock
                                    || err.kind() == std::io::ErrorKind::Interrupted =>
                            {
                                std::thread::sleep(std::time::Duration::from_millis(5));
                            }
                            Err(_) => break,
                        }
                    }
                })
                .expect("live personal bearer thread");
            Self {
                base: format!("http://127.0.0.1:{port}"),
                saw_live,
                saw_expired,
                hits,
                stop,
                thread: Some(thread),
            }
        }

        fn saw_live(&self) -> bool {
            self.saw_live.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn saw_expired(&self) -> bool {
            self.saw_expired.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn hits(&self) -> usize {
            self.hits.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl Drop for LivePersonalBearerListener {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// One stored personal OAuth row is hard-expired. Another personal OAuth
    /// login is live. The personal credits GET uses the live login.
    /// `creditUsagePercent` 10 paints `limits 90%` on the Personal tab and
    /// the header. `$78.41` stays off that body while the included percent
    /// remains below 100.
    #[test]
    #[serial_test::serial]
    fn live_personal_oauth_login_is_the_credits_bearer_when_another_personal_row_is_hard_expired() {
        use crate::actions::ActionRegistry;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::scrollback::render::ScratchBuffer;
        use xai_grok_shell::auth::{AuthMode, GrokAuth, upsert_supergrok_session};

        const OWED: &str = "one personal row hard-expired, another personal OAuth login live, the credits GET bearer is the live login, creditUsagePercent 10 paints `limits 90%`, and the body does not contain `$78.41` while that included percent remains";

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let _cooldown = xai_grok_test_support::EnvGuard::set("GROK_DISABLE_SHARED_RATE_LIMIT", "1");
        clear_account_billing_cache();
        xai_grok_shell::auth::clear_included_billing_cache();
        let server = LivePersonalBearerListener::start();
        let _proxy = AccountBillingProxyGuard::arm(&server.base);
        let expired_at = chrono::Utc::now() - chrono::Duration::days(30);
        let live_at = chrono::Utc::now() + chrono::Duration::days(7);
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            "https://auth.x.ai::aaa-hard-expired",
            GrokAuth {
                key: LivePersonalBearerListener::EXPIRED_TOKEN.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "aaa-hard-expired-personal".into(),
                principal_type: Some("User".into()),
                principal_id: Some("aaa-hard-expired-personal".into()),
                team_id: None,
                expires_at: Some(expired_at),
                refresh_token: None,
                ..GrokAuth::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            "https://auth.x.ai::zzz-live",
            GrokAuth {
                key: LivePersonalBearerListener::LIVE_TOKEN.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "zzz-live-personal".into(),
                principal_type: Some("User".into()),
                principal_id: Some("zzz-live-personal".into()),
                team_id: None,
                expires_at: Some(live_at),
                refresh_token: None,
                ..GrokAuth::default()
            },
        );
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth json"),
        )
        .expect("write auth.json");

        let area = Rect::new(0, 0, 120, 40);
        let mut state = fresh_card_state();
        let buf = render_seeded_card(&mut state, area);
        let personal = account_body_lines(&buf, area, &state).join("\n");
        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let decoy = weekly_bal(99.0, end);
        crate::appearance::cache::set_hide_header(false);
        let mut agent = make_agent();
        agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
        agent.credit_balance = Some(decoy);
        let mut header_buf = ratatui::buffer::Buffer::empty(area);
        let mut scratch = ScratchBuffer::new();
        agent.draw(
            area,
            &mut header_buf,
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
        let header = painted_lines(&header_buf, area).join("\n");
        let body = format!("{personal}\n{header}");
        assert_eq!(
            limits_percent_figure(Some(10.0)).as_deref(),
            Some("limits 90%"),
            "{OWED}"
        );
        assert!(
            server.hits() >= 1 && server.saw_live() && !server.saw_expired(),
            "{OWED}"
        );
        assert!(
            personal.contains("limits 90%")
                && header.contains("limits 90%")
                && !personal.contains("limits 10%")
                && !header.contains("limits 10%")
                && !header.contains("limits 1%")
                && !body.contains("$78.41")
                && !personal.contains("Personal SuperGrok has no session on this process."),
            "{OWED}\npersonal:\n{personal}\nheader:\n{header}"
        );
    }

    struct GrokCreditsConfigUrlGuard;

    impl GrokCreditsConfigUrlGuard {
        fn arm(url: &str) -> Self {
            set_grok_credits_config_url_override(Some(url.to_string()));
            Self
        }
    }

    impl Drop for GrokCreditsConfigUrlGuard {
        fn drop(&mut self) {
            set_grok_credits_config_url_override(None);
            clear_account_billing_cache();
        }
    }

    struct GrokCreditsConfigListener {
        url: String,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl GrokCreditsConfigListener {
        fn start() -> Self {
            use std::sync::atomic::{AtomicBool, Ordering};

            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("credits config listen");
            let port = listener.local_addr().expect("credits config port").port();
            listener
                .set_nonblocking(true)
                .expect("credits config nonblocking");
            let stop = std::sync::Arc::new(AtomicBool::new(false));
            let stop_bg = std::sync::Arc::clone(&stop);
            let body = grok_credits_config_fixture_bytes();
            let thread = std::thread::Builder::new()
                .name("grok-credits-config".to_owned())
                .spawn(move || {
                    while !stop_bg.load(Ordering::SeqCst) {
                        match listener.accept() {
                            Ok((mut stream, _)) => {
                                let _ = stream.set_nonblocking(false);
                                let _ = read_http_headers(&mut stream);
                                let _ = write_grpc_web_http_response(&mut stream, &body);
                            }
                            Err(err)
                                if err.kind() == std::io::ErrorKind::WouldBlock
                                    || err.kind() == std::io::ErrorKind::Interrupted =>
                            {
                                std::thread::sleep(std::time::Duration::from_millis(5));
                            }
                            Err(_) => break,
                        }
                    }
                })
                .expect("grok credits config thread");
            Self {
                url: format!(
                    "http://127.0.0.1:{port}/grok_api_v2.GrokBuildBilling/GetGrokCreditsConfig"
                ),
                stop,
                thread: Some(thread),
            }
        }
    }

    impl Drop for GrokCreditsConfigListener {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn grok_credits_config_fixture_bytes() -> Vec<u8> {
        const FIXTURE: &str = "AAAAAGoKaA0AACBBEgAaACIMCIyckdYGEMD//dcBKgwIjJG21gYQwP/91wE6BwgEFQAA4EA6BwgGFQAAQEA6AggFOgIICEIeCAISDAiMnJHWBhDA//3XARoMCIyRttYGEMD//dcBWAFiAwihPWgBgAAAAA9ncnBjLXN0YXR1czowDQo=";
        fn val(byte: u8) -> u8 {
            match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => 0,
            }
        }
        let bytes = FIXTURE.as_bytes();
        let mut out = Vec::new();
        let mut index = 0;
        while index + 4 <= bytes.len() {
            let packed = (u32::from(val(bytes[index])) << 18)
                | (u32::from(val(bytes[index + 1])) << 12)
                | (u32::from(val(bytes[index + 2])) << 6)
                | u32::from(val(bytes[index + 3]));
            out.push((packed >> 16) as u8);
            if bytes[index + 2] != b'=' {
                out.push((packed >> 8) as u8);
            }
            if bytes[index + 3] != b'=' {
                out.push(packed as u8);
            }
            index += 4;
        }
        out
    }

    fn write_grpc_web_http_response(
        stream: &mut std::net::TcpStream,
        body: &[u8],
    ) -> std::io::Result<()> {
        use std::io::Write;

        let header = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/grpc-web+proto\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(body)?;
        stream.flush()
    }

    /// GetGrokCreditsConfig with 10% used paints `limits 90%` on the Personal
    /// tab and the header. The header does not show `$78.41` while that
    /// included percent remains.
    #[test]
    #[serial_test::serial]
    fn get_grok_credits_config_ten_percent_used_paints_limits_ninety_on_personal_tab_and_header() {
        use crate::actions::ActionRegistry;
        use crate::app::agent_view::test_fixtures::make_agent;
        use crate::app::agent_view::{AppRenderParams, BannerSlotParams};
        use crate::scrollback::render::ScratchBuffer;
        use xai_grok_shell::auth::{AuthMode, GrokAuth, upsert_supergrok_session};

        const OWED: &str = "GetGrokCreditsConfig with 10% used paints limits 90% on the Personal tab and the header, and the header does not show $78.41 while that included percent remains.";

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let _cooldown = xai_grok_test_support::EnvGuard::set("GROK_DISABLE_SHARED_RATE_LIMIT", "1");
        clear_account_billing_cache();
        xai_grok_shell::auth::clear_included_billing_cache();
        set_grok_credits_config_url_override(None);
        crate::appearance::cache::set_hide_header(false);

        let unread_area = Rect::new(0, 0, 200, 40);
        let mut unread_agent = make_agent();
        unread_agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
        unread_agent.credit_balance = None;
        let mut unread_buf = ratatui::buffer::Buffer::empty(unread_area);
        let mut unread_scratch = ScratchBuffer::new();
        unread_agent.draw(
            unread_area,
            &mut unread_buf,
            &ActionRegistry::defaults(),
            &mut unread_scratch,
            None,
            false,
            BannerSlotParams::none(),
            false,
            false,
            &mut Vec::new(),
            AppRenderParams::default(),
        );
        let unread = painted_lines(&unread_buf, unread_area).join("\n");
        assert!(
            unread.contains("Limits and Credits")
                && !unread.contains("limits 0%")
                && !unread.contains("$0"),
            "{OWED}\nunread:\n{unread}"
        );
        assert_eq!(
            limits_percent_figure(Some(28.0)).as_deref(),
            Some("limits 72%"),
            "{OWED}"
        );

        let server = GrokCreditsConfigListener::start();
        let _url = GrokCreditsConfigUrlGuard::arm(&server.url);
        let expired_at = chrono::Utc::now() - chrono::Duration::days(30);
        let live_at = chrono::Utc::now() + chrono::Duration::days(7);
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            "https://auth.x.ai::aaa-hard-expired",
            GrokAuth {
                key: LivePersonalBearerListener::EXPIRED_TOKEN.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "aaa-hard-expired-personal".into(),
                principal_type: Some("User".into()),
                principal_id: Some("aaa-hard-expired-personal".into()),
                team_id: None,
                expires_at: Some(expired_at),
                refresh_token: None,
                ..GrokAuth::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            "https://auth.x.ai::zzz-live",
            GrokAuth {
                key: LivePersonalBearerListener::LIVE_TOKEN.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "zzz-live-personal".into(),
                principal_type: Some("User".into()),
                principal_id: Some("zzz-live-personal".into()),
                team_id: None,
                expires_at: Some(live_at),
                refresh_token: None,
                ..GrokAuth::default()
            },
        );
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth json"),
        )
        .expect("write auth.json");

        let area = Rect::new(0, 0, 120, 40);
        let mut state = fresh_card_state();
        let buf = render_seeded_card(&mut state, area);
        let personal = account_body_lines(&buf, area, &state).join("\n");
        let end = DateTime::parse_from_rfc3339("2026-10-12T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let decoy = weekly_bal(99.0, end);
        let mut agent = make_agent();
        agent.sampling_identity = SamplingIdentityKind::SuperGrokSession;
        agent.credit_balance = Some(decoy);
        let mut header_buf = ratatui::buffer::Buffer::empty(area);
        let mut scratch = ScratchBuffer::new();
        agent.draw(
            area,
            &mut header_buf,
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
        let header = painted_lines(&header_buf, area).join("\n");
        assert!(
            personal.contains("limits 90%") && !personal.contains("limits 10%"),
            "{OWED}\npersonal:\n{personal}"
        );
        assert!(
            header.contains("limits 90%")
                && !header.contains("$78.41")
                && !header.contains("limits 0%")
                && !header.contains("limits 10%"),
            "{OWED}\nheader:\n{header}"
        );
    }

    /// `Using personal limits` and `Using limits` are not both footer labels.
    /// A dead session keeps `Using limits`. A live session paints
    /// `Using personal limits` and drops the exact `Using limits` label.
    #[test]
    #[serial_test::serial]
    fn footer_does_not_pair_using_personal_limits_with_using_limits() {
        use xai_grok_shell::auth::limits_pins::{LimitsPins, MeterSource, save_limits_pins};
        use xai_grok_shell::auth::{
            AuthMode, GrokAuth, SupergrokIdentityPin, upsert_supergrok_session,
        };

        fn labels() -> Vec<&'static str> {
            painted_shortcuts(&fresh_card_state())
                .into_iter()
                .map(|shortcut| shortcut.label)
                .collect()
        }
        fn has_label(labels: &[&str], want: &str) -> bool {
            labels.contains(&want)
        }
        fn write_session(home: &std::path::Path, auth: GrokAuth) {
            let base = "https://auth.x.ai::footer-using-personal";
            let mut map = std::collections::BTreeMap::new();
            upsert_supergrok_session(&mut map, base, auth);
            std::fs::write(
                home.join("auth.json"),
                serde_json::to_vec_pretty(&map).expect("auth json"),
            )
            .expect("write auth.json");
        }

        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let expired = chrono::Utc::now() - chrono::Duration::days(1);
        write_session(
            home.path(),
            GrokAuth {
                key: "not-a-jwt".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-dead-personal".into(),
                principal_type: Some("User".into()),
                principal_id: Some("u-dead-personal".into()),
                team_id: None,
                expires_at: Some(expired),
                refresh_token: None,
                ..GrokAuth::default()
            },
        );
        save_limits_pins(&LimitsPins {
            meter_source: Some(MeterSource::Included),
            supergrok_identity: Some(SupergrokIdentityPin::Personal),
            use_console: false,
            stay_supergrok: true,
        })
        .expect("included personal pin");
        let body = account_section_lines(
            xai_grok_shell::auth::SupergrokAccountRole::Personal,
            Utc::now(),
        )
        .join("\n");
        assert!(
            body.contains("Personal SuperGrok has no session on this process."),
            "a hard-expired personal session is not a session on this process:\n{body}"
        );
        let dead = labels();
        assert!(
            !has_label(&dead, "Using personal limits") && !has_label(&dead, "Use personal limits"),
            "a dead personal session does not paint Using personal limits or Use personal limits: {dead:?}"
        );
        assert!(
            has_label(&dead, "Using limits"),
            "a dead personal session keeps Using limits: {dead:?}"
        );

        let live = chrono::Utc::now() + chrono::Duration::days(7);
        write_session(
            home.path(),
            GrokAuth {
                key: "live.personal.session".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-dead-personal".into(),
                principal_type: Some("User".into()),
                principal_id: Some("u-dead-personal".into()),
                team_id: None,
                expires_at: Some(live),
                refresh_token: None,
                ..GrokAuth::default()
            },
        );
        // A two-dot key would HTTP the proxy. This half only reads the footer.
        let live_labels = labels();
        assert!(
            has_label(&live_labels, "Using personal limits"),
            "a live personal session paints Using personal limits: {live_labels:?}"
        );
        assert!(
            !has_label(&live_labels, "Using limits"),
            "a live personal session does not also paint Using limits: {live_labels:?}"
        );
        assert!(
            !has_label(&live_labels, "Use personal limits"),
            "Using personal limits is the one copy of that action: {live_labels:?}"
        );
    }

    /// Only the account the next request draws says Using. The other stored
    /// login stays listed and does not say Using. Switching Personal or
    /// Business changes which one says Using and changes the next request.
    /// It does not light both.
    #[test]
    #[serial_test::serial]
    fn personal_and_business_cannot_both_say_using() {
        use crossterm::event::{MouseButton, MouseEventKind};
        use xai_grok_shell::auth::limits_pins::{
            LimitsPins, MeterSource, load_limits_pins, save_limits_pins,
        };
        use xai_grok_shell::auth::{
            AuthMode, GrokAuth, SupergrokIdentityPin, upsert_supergrok_session,
        };

        let owed = "only the account the next request draws says Using. The other stored login stays listed and does not say Using. Switching Personal or Business changes which one says Using and changes the next request. It does not light both.";
        let personal_email = "person@example.test";
        let business_email = "hunter@surmount.systems";
        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let base = "https://auth.x.ai::both-using";
        let live = chrono::Utc::now() + chrono::Duration::days(7);
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "personal-session-do-not-print".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "personal-user".into(),
                email: Some(personal_email.into()),
                principal_type: Some("User".into()),
                team_id: None,
                expires_at: Some(live),
                refresh_token: None,
                ..GrokAuth::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "business-session-do-not-print".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "business-user".into(),
                email: Some(business_email.into()),
                principal_type: Some("Team".into()),
                principal_id: Some("team-surmount".into()),
                team_id: Some("team-surmount".into()),
                expires_at: Some(live),
                refresh_token: None,
                ..GrokAuth::default()
            },
        );
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth json"),
        )
        .expect("write auth.json");
        save_limits_pins(&LimitsPins {
            meter_source: Some(MeterSource::Included),
            supergrok_identity: Some(SupergrokIdentityPin::Business),
            use_console: false,
            stay_supergrok: true,
        })
        .expect("business included pin");

        let area = Rect::new(0, 0, 120, 40);
        let paint = |state: &mut LimitsModalState| -> String {
            let mut buf = Buffer::empty(area);
            render_limits_modal(
                &mut buf,
                area,
                state,
                &Theme::default(),
                false,
                chrono::Utc::now(),
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
        let says_both = |screen: &str| {
            screen.contains("Using personal limits") && screen.contains("Using business limits")
        };

        let mut state = fresh_card_state();
        assert_eq!(
            state.window.active_tab, CREDITS_TAB,
            "{owed} the card opens on the Business tab when the next request draws business included SuperGrok period limits"
        );
        state.window.active_tab = LIMITS_TAB;
        let personal_while_business_is_next = paint(&mut state);
        assert!(
            personal_while_business_is_next.contains(&format!("{personal_email} - personal"))
                && !personal_while_business_is_next
                    .contains(&format!("{personal_email} - personal (current)")),
            "{owed} personal remains listed and is not the next request:\n{personal_while_business_is_next}"
        );
        assert!(
            personal_while_business_is_next
                .contains(&format!("{business_email} - business (current)")),
            "{owed} the business row is current:\n{personal_while_business_is_next}"
        );
        assert!(
            !personal_while_business_is_next.contains("Using limits")
                && !personal_while_business_is_next.contains("Using personal limits")
                && !personal_while_business_is_next.contains("Using business limits"),
            "{owed} the Personal tab does not say Using while business is the next request:\n{personal_while_business_is_next}"
        );

        state.window.active_tab = CREDITS_TAB;
        let business_screen = paint(&mut state);
        assert!(
            business_screen.contains("Using business limits")
                && business_screen.matches("Using business limits").count() == 1
                && !business_screen.contains("Use business limits")
                && !business_screen.contains("Using credits")
                && !business_screen.contains("Using limits")
                && !business_screen.contains("Using personal limits")
                && !says_both(&business_screen),
            "{owed} only Business says Using, and Using business limits exists once:\n{business_screen}"
        );
        assert!(
            business_screen.contains(&format!("{personal_email} - personal"))
                && !business_screen.contains(&format!("{personal_email} - personal (current)")),
            "{owed} the other stored login stays listed:\n{business_screen}"
        );

        let personal_tab = state.window.tab_rects[LIMITS_TAB].expect("Personal tab");
        let switched_personal = handle_limits_mouse(
            &mut state,
            MouseEventKind::Down(MouseButton::Left),
            personal_tab.x,
            personal_tab.y,
        );
        assert_eq!(switched_personal, LimitsModalOutcome::Changed);
        let personal_pin = load_limits_pins();
        assert_eq!(
            personal_pin.supergrok_identity,
            Some(SupergrokIdentityPin::Personal)
        );
        assert_eq!(personal_pin.meter_source, Some(MeterSource::Included));
        assert!(!personal_pin.use_console);
        let personal_screen = paint(&mut state);
        assert!(
            personal_screen.contains("Using personal limits")
                && !personal_screen.contains("Using limits")
                && !personal_screen.contains("Using business limits")
                && !says_both(&personal_screen),
            "{owed} switching to Personal makes only that account say Using:\n{personal_screen}"
        );
        assert!(
            personal_screen.contains(&format!("{personal_email} - personal (current)"))
                && personal_screen.contains(&format!("{business_email} - business"))
                && !personal_screen.contains(&format!("{business_email} - business (current)")),
            "{owed} personal is the next request and business stays listed:\n{personal_screen}"
        );

        let business_tab = state.window.tab_rects[CREDITS_TAB].expect("Business tab");
        let switched_business = handle_limits_mouse(
            &mut state,
            MouseEventKind::Down(MouseButton::Left),
            business_tab.x,
            business_tab.y,
        );
        assert_eq!(switched_business, LimitsModalOutcome::Changed);
        let business_pin = load_limits_pins();
        assert_eq!(
            business_pin.supergrok_identity,
            Some(SupergrokIdentityPin::Business)
        );
        assert_eq!(business_pin.meter_source, Some(MeterSource::Included));
        assert!(!business_pin.use_console);
        let only_business = paint(&mut state);
        assert!(
            only_business.contains("Using business limits")
                && !only_business.contains("Using limits")
                && !only_business.contains("Using personal limits")
                && !says_both(&only_business)
                && only_business.contains(&format!("{business_email} - business (current)"))
                && only_business.contains(&format!("{personal_email} - personal"))
                && !only_business.contains(&format!("{personal_email} - personal (current)")),
            "{owed} switching to Business makes only that account say Using:\n{only_business}"
        );
    }

    /// The same email on the personal token and the business token stays two
    /// rows, and the Business tab does not label the business token personal.
    /// The Personal tab does not label the personal token business. The email
    /// address does not decide the slot.
    #[test]
    #[serial_test::serial]
    fn same_email_on_two_oauth_tokens_keeps_each_slot_label() {
        use crossterm::event::{MouseButton, MouseEventKind};
        use xai_grok_shell::auth::limits_pins::{
            LimitsPins, MeterSource, load_limits_pins, save_limits_pins,
        };
        use xai_grok_shell::auth::{
            AuthMode, GrokAuth, SupergrokIdentityPin, upsert_supergrok_session,
        };

        let owed = "the same email on the personal token and the business token stays two rows, and the Business tab does not label the business token personal. The Personal tab does not label the personal token business. Only the account the next request draws says Using. A Personal or Business click still changes the next request. use_console stays false unless the console row is the next request.";
        let email = "same.oauth@example.test";
        let personal_label = format!("{email} - personal");
        let business_label = format!("{email} - business");
        let home = tempfile::TempDir::new().expect("temp GROK_HOME");
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let base = "https://auth.x.ai::same-email-two-tokens";
        let live = chrono::Utc::now() + chrono::Duration::days(7);
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "personal-token-do-not-print".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "personal-slot-user".into(),
                email: Some(email.into()),
                principal_type: Some("User".into()),
                team_id: None,
                expires_at: Some(live),
                refresh_token: None,
                ..GrokAuth::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "business-token-do-not-print".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "business-slot-user".into(),
                email: Some(email.into()),
                principal_type: Some("Team".into()),
                principal_id: Some("team-shared-slot".into()),
                team_id: Some("team-shared-slot".into()),
                expires_at: Some(live),
                refresh_token: None,
                ..GrokAuth::default()
            },
        );
        map.insert(
            "xai::api_key".into(),
            GrokAuth {
                key: "console-key-do-not-print".into(),
                auth_mode: AuthMode::ApiKey,
                user_id: String::new(),
                ..GrokAuth::default()
            },
        );
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth json"),
        )
        .expect("write auth.json");
        save_limits_pins(&LimitsPins {
            meter_source: Some(MeterSource::Included),
            supergrok_identity: Some(SupergrokIdentityPin::Business),
            use_console: false,
            stay_supergrok: true,
        })
        .expect("business included pin");

        let area = Rect::new(0, 0, 120, 40);
        let paint = |state: &mut LimitsModalState| -> String {
            let mut buf = Buffer::empty(area);
            render_limits_modal(
                &mut buf,
                area,
                state,
                &Theme::default(),
                false,
                chrono::Utc::now(),
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
        let says_both = |screen: &str| {
            screen.contains("Using personal limits") && screen.contains("Using business limits")
        };

        let mut state = fresh_card_state();
        assert_eq!(
            state.window.active_tab, CREDITS_TAB,
            "{owed} the card opens on the Business tab when the next request draws business included SuperGrok period limits"
        );
        let business_screen = paint(&mut state);
        assert!(
            business_screen.matches(&personal_label).count() == 1
                && business_screen.matches(&business_label).count() == 1,
            "{owed} the same email stays two rows:\n{business_screen}"
        );
        assert!(
            business_screen.contains(&format!("{business_label} (current)"))
                && !business_screen.contains(&format!("{personal_label} (current)")),
            "{owed} the Business tab does not label the business token personal:\n{business_screen}"
        );
        assert!(
            business_screen.contains("Using business limits")
                && business_screen.matches("Using business limits").count() == 1
                && !business_screen.contains("Use business limits")
                && !business_screen.contains("Using credits")
                && !business_screen.contains("Using limits")
                && !business_screen.contains("Using personal limits")
                && !says_both(&business_screen),
            "{owed} only the business account says Using, and Using business limits exists once:\n{business_screen}"
        );
        assert!(
            business_screen.contains("console API key") && business_screen.contains("3 sessions"),
            "{owed} the console API key and the session count stay:\n{business_screen}"
        );
        for secret in [
            "personal-token-do-not-print",
            "business-token-do-not-print",
            "console-key-do-not-print",
        ] {
            assert!(
                !business_screen.contains(secret),
                "the Accounts list does not print {secret}"
            );
        }

        let personal_tab = state.window.tab_rects[LIMITS_TAB].expect("Personal tab");
        let switched_personal = handle_limits_mouse(
            &mut state,
            MouseEventKind::Down(MouseButton::Left),
            personal_tab.x,
            personal_tab.y,
        );
        assert_eq!(switched_personal, LimitsModalOutcome::Changed);
        let personal_pin = load_limits_pins();
        assert_eq!(
            personal_pin.supergrok_identity,
            Some(SupergrokIdentityPin::Personal)
        );
        assert!(!personal_pin.use_console);
        let personal_screen = paint(&mut state);
        assert!(
            personal_screen.contains(&format!("{personal_label} (current)"))
                && personal_screen.contains(&business_label)
                && !personal_screen.contains(&format!("{business_label} (current)"))
                && personal_screen.matches(&personal_label).count() == 1
                && personal_screen.matches(&business_label).count() == 1,
            "{owed} the Personal tab does not label the personal token business:\n{personal_screen}"
        );
        assert!(
            personal_screen.contains("Using personal limits")
                && !personal_screen.contains("Using limits")
                && !personal_screen.contains("Using business limits")
                && !says_both(&personal_screen),
            "{owed} only the personal account says Using:\n{personal_screen}"
        );
        assert!(
            personal_screen.contains("console API key") && personal_screen.contains("3 sessions"),
            "{owed} the console API key and the session count stay:\n{personal_screen}"
        );

        let business_tab = state.window.tab_rects[CREDITS_TAB].expect("Business tab");
        let switched_business = handle_limits_mouse(
            &mut state,
            MouseEventKind::Down(MouseButton::Left),
            business_tab.x,
            business_tab.y,
        );
        assert_eq!(switched_business, LimitsModalOutcome::Changed);
        let business_pin = load_limits_pins();
        assert_eq!(
            business_pin.supergrok_identity,
            Some(SupergrokIdentityPin::Business)
        );
        assert_eq!(business_pin.meter_source, Some(MeterSource::Included));
        assert!(!business_pin.use_console);
        let only_business = paint(&mut state);
        assert!(
            only_business.contains("Using business limits")
                && !only_business.contains("Using limits")
                && !only_business.contains("Using personal limits")
                && !says_both(&only_business)
                && only_business.contains(&format!("{business_label} (current)"))
                && only_business.contains(&personal_label)
                && !only_business.contains(&format!("{personal_label} (current)"))
                && only_business.contains("console API key")
                && only_business.contains("3 sessions"),
            "{owed} switching to Business makes only that account say Using:\n{only_business}"
        );
    }
}
