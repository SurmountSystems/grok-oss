//! Honesty copy for SuperGrok limits surfaces (`/limits`, `grok limits`).
//!
//! Exact user-facing phrases live here so unit tests can assert them without
//! scattering string literals. Meters stay distinct: SuperGrok included weekly
//! % ≠ SuperGrok $ extras ≠ console team prepaid ≠ team postpaid OAuth/API ≠
//! team default credits (dashboard allotment).
//!
//! **Named contracts:**
//! - Do not present SuperGrok included % as proven included-limit burn.
//! - Optional flat-poll note only names meters that were **observed** flat
//!   (never claim Build product % or SuperGrok $ extras stayed flat when those
//!   fields were absent on the poll series).
//! - C6: when SuperGrok session is live and team postpaid OAuth class dominates,
//!   plain English that session can still move team Usage dollars.
//! - When console team prepaid dollars are shown, name the ≤Ns process-cache
//!   lag and that `grok limits` / TUI `/limits` force a fresh Management fetch.

use super::credit_bar::SamplingIdentityKind;

/// SuperGrok included % is a billing poll reading, not proof of burn.
///
/// Shown when live sampling is SuperGrok session and an included reading is
/// present. Plain American English; no em dash.
pub const NOTE_INCLUDED_PCT_IS_BILLING_POLL: &str = "Note: SuperGrok included % is the billing \
poll reading, not proof of included-limit burn.";

/// SuperGrok session can still move team Usage dollars via OAuth class.
///
/// Ideal C6 / branch 2b. Shown when live sampling is SuperGrok session and
/// Management postpaid preview shows OAuth class strictly above API class
/// (and &gt; 0). Distinct from console ApiKey live, from prepaid ledger
/// remaining, and from SuperGrok included weekly debit proof. Does **not**
/// mean SuperGrok dollar extras are the live driver or that free SuperGrok
/// period moved.
pub const NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS: &str = "Note: SuperGrok session can still \
move team Usage dollars (OAuth / Grok Build class on the team invoice) without proving \
SuperGrok included weekly moved, even when the console API key is not live. That settlement \
rise is not free SuperGrok period burn proof and not SuperGrok dollar extras as the live driver.";

/// Flat free SuperGrok period + rising team OAuth / Grok Build settlement.
///
/// Shown when SuperGrok is live, flat-poll evidence is set, and OAuth postpaid
/// dominates. Strengthens C6: names that team Grok Build class can climb while
/// free period stays flat, without calling that class SuperGrok extras.
pub const NOTE_FLAT_FREE_PERIOD_SETTLEMENT_RISE_NOT_EXTRAS: &str = "Note: free SuperGrok period \
can stay flat across recent polls while team Grok Build / OAuth settlement dollars rise under \
SuperGrok session; product does not invent free-period debit and does not treat team settlement \
as SuperGrok dollar extras.";

/// Console team prepaid lag honesty (when dollars are shown).
///
/// Background TUI polls reuse a warm Management process entry for up to
/// [`xai_grok_shell::auth::CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS`] seconds.
/// Explicit `grok limits` collect and TUI `/limits` open bust that process
/// cache. App/TUI state may also keep last successful cents when a later
/// fetch returns `None` (older than the process TTL). Names both so operators
/// are not misled by "up to Ns" alone.
pub fn note_console_team_prepaid_may_lag() -> String {
    let secs = xai_grok_shell::auth::CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS;
    format!(
        "Note: console team prepaid process cache may lag up to {secs}s; the UI may also \
keep last successful cents until a later successful fetch. Running grok limits or opening \
/limits forces a fresh Management fetch."
    )
}

/// Team default credits are a separate dashboard allotment meter.
///
/// Shown when postpaid preview carried `defaultCredits`. Not the console team
/// prepaid wallet, not free SuperGrok period allowance, not SuperGrok top-up
/// dollars.
pub const NOTE_TEAM_DEFAULT_CREDITS_ARE_DASHBOARD_ALLOTMENT: &str = "Note: team default credits \
are the console dashboard allotment (postpaid preview defaultCredits), not the team prepaid \
wallet, not free SuperGrok period allowance, and not SuperGrok prepaid top-up dollars.";

/// Platforms → Grok Business → licenses Usage (messages / conversations) is
/// not dogfood proof for this CLI.
///
/// Zeros on that license page are **expected** for CLI SuperGrok (this client
/// does not drive seat message/conversation counters). Real burn shows as
/// **team Usage** dollars (browser team Usage / spend / Grok Build) and on
/// SuperGrok included % / extras plus team prepaid / postpaid OAuth / usage
/// series when a management key is set.
pub const NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER: &str = "Note: the console Platforms → Grok \
Business licenses page (messages / conversations) is not dogfood proof for this product. \
CLI SuperGrok does not drive seat message/conversation counters; zeros there are expected. \
Real burn shows as team Usage dollars (browser team Usage / spend / Grok Build) and on \
SuperGrok included % / extras plus team prepaid / postpaid OAuth / usage series when a \
management key is set.";

/// Doctor human block: wrong browser page vs right dogfood proof surfaces.
///
/// Counts/fingerprints stay in dual-auth status; this block is plain meter map
/// only (no secrets). Always ends with a trailing newline for doctor append.
pub fn dogfood_burn_proof_doctor_block() -> String {
    "Dogfood burn proof (meters stay distinct)\n\
  Proof: grok-oss limits / TUI /limits team postpaid OAuth (Grok Build class) \
and usage series when a management key is set; browser team Usage \
(console.x.ai team .../usage spend charts).\n\
  Not proof: Platforms → Grok Business → licenses Usage (messages / \
conversations / active users). CLI SuperGrok does not drive those seat \
counters; zeros there are expected.\n"
        .to_string()
}

/// Shared Grok Build productUsage line (human `/limits` and `/usage`).
///
/// Floors like included %. Always ends with `% used` so surfaces match.
/// Distinct from top-level included allowance %. Never invent when absent.
pub fn format_grok_build_product_usage_line(pct: f64) -> String {
    format!("Grok Build product usage: {}% used", pct.floor() as i64)
}

/// Dual SuperGrok: one principal's JWT failed billing; free-period % on the
/// empty row may be shared-pool fill (not a successful poll of that login).
///
/// `failed_role` is `personal` / `business`. Names re-login path.
pub fn note_dual_principal_billing_failed(failed_role: &str) -> String {
    let role = failed_role.trim();
    let role = if role.is_empty() { "unknown" } else { role };
    format!(
        "Note: SuperGrok ({role}) billing poll failed this run. Re-login that \
SuperGrok account with: grok login"
    )
}

/// Dual SuperGrok: free-period % (and Extra Usage Credits when filled) on a
/// row came from the shared SuperGrok pool, not a successful poll of that JWT.
pub fn note_shared_pool_fill_not_live_poll(filled_role: &str) -> String {
    let role = filled_role.trim();
    let role = if role.is_empty() {
        "a SuperGrok login"
    } else {
        role
    };
    format!(
        "Note: SuperGrok ({role}) free-period % (and Extra Usage Credits when \
shown) comes from the shared SuperGrok pool, not a successful billing poll of \
that login this run."
    )
}

/// Phrases that must never appear as consumption claims from flat % alone.
pub const FORBIDDEN_INCLUDED_BURN_CLAIMS: &[&str] = &[
    "using SuperGrok limits",
    "burning included limits",
    "you are burning included",
];

/// Flat-poll honesty note naming only meters that were observed flat.
///
/// Always names SuperGrok included % (required for the detector). Optionally
/// names Grok Build product % and SuperGrok $ extras when those fields were
/// present on every sample in the flat window. Does **not** claim Build or
/// extras stayed flat when they were never on the wire.
pub fn flat_poll_unproven_debit_note(observed_build: bool, observed_extras: bool) -> String {
    let mut parts: Vec<&str> = vec!["SuperGrok included %"];
    if observed_build {
        parts.push("Grok Build product %");
    }
    if observed_extras {
        parts.push("SuperGrok $ extras");
    }
    let meters = match parts.as_slice() {
        [a] => (*a).to_string(),
        [a, b] => format!("{a} and {b}"),
        [a, b, c] => format!("{a}, {b}, and {c}"),
        other => other.join(", "),
    };
    format!(
        "Note: {meters} stayed flat across recent polls; included debit is unproven \
(session path can still be live)."
    )
}

/// Inputs for SuperGrok limits honesty notes (pure; hermetic tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LimitsHonestyInput {
    /// Live sampling identity (session path vs console key).
    pub live: SamplingIdentityKind,
    /// True when any SuperGrok principal row has an included % reading.
    pub has_included_reading: bool,
    /// True when product observed flat SuperGrok meters across recent polls
    /// (caller-supplied evidence; do not invent).
    pub flat_poll_unproven_debit: bool,
    /// True when every sample in the flat window carried Build product %
    /// (and it stayed flat). Only meaningful when
    /// [`Self::flat_poll_unproven_debit`] is true.
    pub flat_poll_observed_build: bool,
    /// True when every sample in the flat window carried SuperGrok $ extras
    /// (and it stayed flat). Only meaningful when
    /// [`Self::flat_poll_unproven_debit`] is true.
    pub flat_poll_observed_extras: bool,
    /// True when Management postpaid preview shows OAuth class dominating
    /// API class (caller-supplied; do not invent from SuperGrok % alone).
    pub oauth_postpaid_dominates: bool,
    /// True when console team prepaid dollars are shown (Management meter).
    /// Independent of SuperGrok vs console live identity.
    pub has_console_team_prepaid_reading: bool,
    /// True when team default credits (dashboard allotment) are shown.
    pub has_team_default_credits_reading: bool,
}

/// Build honesty notes for limits modal / human `grok limits` (ordered).
///
/// - Always: license page ≠ SuperGrok / team Management (no invented license
///   message/conversation counts).
/// - Prepaid lag note when console team prepaid dollars are shown (any live
///   identity). Names process-cache lag + `grok limits` / `/limits` force-refresh.
/// - Base note when SuperGrok session is live and included % is shown.
/// - Flat-poll note only when SuperGrok session is live **and**
///   [`LimitsHonestyInput::flat_poll_unproven_debit`]. Meter names come from
///   observed flags (no invent Build/extras flat claim).
/// - C6 team Usage note when SuperGrok session is live **and**
///   [`LimitsHonestyInput::oauth_postpaid_dominates`].
/// - Console-live: no SuperGrok burn / flat-poll / C6 honesty notes (prepaid
///   lag note still allowed when dollars are shown; license note still present).
pub fn honesty_notes_for_limits(input: LimitsHonestyInput) -> Vec<String> {
    let mut notes = Vec::new();
    // Always: license seat page is not a product meter (plain dogfood honesty).
    notes.push(NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER.to_string());
    if input.has_console_team_prepaid_reading {
        notes.push(note_console_team_prepaid_may_lag());
    }
    if input.has_team_default_credits_reading {
        notes.push(NOTE_TEAM_DEFAULT_CREDITS_ARE_DASHBOARD_ALLOTMENT.to_string());
    }
    if input.live.is_console() {
        return notes;
    }
    if input.has_included_reading {
        notes.push(NOTE_INCLUDED_PCT_IS_BILLING_POLL.to_string());
    }
    if input.flat_poll_unproven_debit {
        notes.push(flat_poll_unproven_debit_note(
            input.flat_poll_observed_build,
            input.flat_poll_observed_extras,
        ));
    }
    if input.oauth_postpaid_dominates {
        notes.push(NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS.to_string());
    }
    // Flat free period + settlement rise: strengthen honesty so operators do
    // not read team Grok Build $ climb as SuperGrok extras or free-period move.
    if input.flat_poll_unproven_debit && input.oauth_postpaid_dominates {
        notes.push(NOTE_FLAT_FREE_PERIOD_SETTLEMENT_RISE_NOT_EXTRAS.to_string());
    }
    notes
}

/// True when `text` contains a forbidden overclaim phrase (ASCII
/// case-insensitive so title-case / shouty overclaims still match).
pub fn contains_forbidden_included_burn_claim(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    FORBIDDEN_INCLUDED_BURN_CLAIMS
        .iter()
        .any(|p| lower.contains(&p.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_base() -> LimitsHonestyInput {
        LimitsHonestyInput {
            live: SamplingIdentityKind::SuperGrokSession,
            has_included_reading: true,
            flat_poll_unproven_debit: false,
            flat_poll_observed_build: false,
            flat_poll_observed_extras: false,
            oauth_postpaid_dominates: false,
            has_console_team_prepaid_reading: false,
            has_team_default_credits_reading: false,
        }
    }

    /// Named contract (Item 5b): default credits note names what the meter is not.
    #[test]
    fn default_credits_note_when_reading_present() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            has_team_default_credits_reading: true,
            has_included_reading: false,
            ..Default::default()
        });
        assert!(
            notes
                .iter()
                .any(|n| n == NOTE_TEAM_DEFAULT_CREDITS_ARE_DASHBOARD_ALLOTMENT),
            "must emit default-credits honesty: {notes:?}"
        );
        let n = notes
            .iter()
            .find(|n| n.as_str() == NOTE_TEAM_DEFAULT_CREDITS_ARE_DASHBOARD_ALLOTMENT)
            .expect("note");
        assert!(
            n.contains("not the team prepaid wallet"),
            "must exclude prepaid wallet: {n}"
        );
        assert!(
            n.contains("not free SuperGrok period allowance"),
            "must exclude free SuperGrok period allowance: {n}"
        );
        assert!(
            n.contains("not SuperGrok prepaid top-up"),
            "must exclude SuperGrok top-up: {n}"
        );
    }

    #[test]
    fn base_note_when_supergrok_live_with_included_reading() {
        let notes = honesty_notes_for_limits(input_base());
        assert!(
            notes
                .iter()
                .any(|n| n.as_str() == NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER),
            "license page honesty always present: {notes:?}"
        );
        assert!(
            notes
                .iter()
                .any(|n| n.as_str() == NOTE_INCLUDED_PCT_IS_BILLING_POLL),
            "included poll honesty: {notes:?}"
        );
        let included = notes
            .iter()
            .find(|n| n.as_str() == NOTE_INCLUDED_PCT_IS_BILLING_POLL)
            .expect("included note");
        assert!(
            included.contains("billing poll reading"),
            "must name poll reading: {included}"
        );
        assert!(
            included.contains("not proof of included-limit burn"),
            "must deny burn proof: {included}"
        );
        assert!(
            !contains_forbidden_included_burn_claim(included),
            "honesty note must not overclaim: {included}"
        );
    }

    #[test]
    fn no_supergrok_burn_notes_when_console_live() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::ConsoleKey,
            has_included_reading: true,
            ..Default::default()
        });
        assert!(
            notes
                .iter()
                .any(|n| n.as_str() == NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER),
            "license page honesty still present on console live: {notes:?}"
        );
        assert!(
            !notes
                .iter()
                .any(|n| n.as_str() == NOTE_INCLUDED_PCT_IS_BILLING_POLL),
            "console live must not sell SuperGrok burn notes: {notes:?}"
        );
        assert!(
            !notes
                .iter()
                .any(|n| n.contains("included debit is unproven")),
            "console live must not sell flat-poll SuperGrok notes: {notes:?}"
        );
    }

    /// Named contract: license page (messages/conversations) is not a product
    /// meter; note names SuperGrok and team Management as the real surfaces.
    #[test]
    fn license_page_note_never_claims_messages_conversations_as_product_meter() {
        let note = NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER;
        let lower = note.to_ascii_lowercase();
        assert!(lower.contains("license"), "must name license page: {note}");
        assert!(
            lower.contains("messages") || lower.contains("conversations"),
            "must name the license chart metrics it is not: {note}"
        );
        assert!(
            lower.contains("not")
                && (lower.contains("dogfood")
                    || lower.contains("supergrok")
                    || lower.contains("management")),
            "must say license page is not dogfood proof / product meter: {note}"
        );
        // Product does not invent license message counts as a live meter claim.
        assert!(
            !lower.contains("license messages used")
                && !lower.contains("seat messages:")
                && !lower.contains("% of license"),
            "must not claim license consumption as product meter: {note}"
        );
        let notes = honesty_notes_for_limits(input_base());
        assert!(
            notes.iter().any(|n| n.as_str() == note),
            "license note on SuperGrok live stack: {notes:?}"
        );
    }

    /// Named contract (P0): license honesty names team Usage / Grok Build as
    /// the real settlement proof, not only "not a SuperGrok meter."
    #[test]
    fn license_honesty_names_team_usage_and_zeros_expected() {
        let note = NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER;
        let lower = note.to_ascii_lowercase();
        assert!(
            lower.contains("team usage"),
            "must name team Usage as settlement surface: {note}"
        );
        assert!(
            lower.contains("grok build") || lower.contains("postpaid"),
            "must name Grok Build class or postpaid: {note}"
        );
        assert!(
            lower.contains("zeros") && lower.contains("expected"),
            "must say license zeros are expected for CLI SuperGrok: {note}"
        );
        assert!(
            lower.contains("dogfood proof") || lower.contains("not dogfood"),
            "must deny license page as dogfood proof: {note}"
        );
        assert!(
            !note.contains('\u{2014}') && !note.contains('—') && !note.contains('\u{2026}'),
            "no em dash / unicode ellipsis: {note}"
        );
        let notes = honesty_notes_for_limits(input_base());
        assert!(
            notes.iter().any(|n| n.as_str() == note),
            "sharper license note on stack: {notes:?}"
        );
    }

    /// Named contract (P0): doctor dogfood block names licenses not proof AND
    /// team Usage / Grok Build class as settlement proof.
    #[test]
    fn doctor_dogfood_block_names_wrong_page_and_right_proof() {
        let block = dogfood_burn_proof_doctor_block();
        let lower = block.to_ascii_lowercase();
        assert!(
            lower.contains("not proof") || lower.contains("not dogfood"),
            "must label wrong page: {block}"
        );
        assert!(
            lower.contains("license")
                && (lower.contains("messages") || lower.contains("conversations")),
            "must name licenses page metrics: {block}"
        );
        assert!(
            lower.contains("zeros") && lower.contains("expected"),
            "must say zeros expected: {block}"
        );
        assert!(
            lower.contains("team usage") || lower.contains("team postpaid"),
            "must name team Usage / postpaid proof: {block}"
        );
        assert!(
            lower.contains("grok build"),
            "must name Grok Build class: {block}"
        );
        assert!(
            lower.contains("/limits") || lower.contains("limits"),
            "must name product limits path: {block}"
        );
        assert!(
            !block.contains("xai-") && !block.contains("sk-"),
            "must not dump secrets: {block}"
        );
        assert!(
            !block.contains('\u{2014}') && !block.contains('—') && !block.contains('\u{2026}'),
            "no em dash / unicode ellipsis: {block}"
        );
    }

    /// Named contract: when console team prepaid $ is shown, name process-cache
    /// lag (TTL), app last-good that can outlive TTL, and that `grok limits`
    /// or TUI `/limits` forces a fresh Management fetch. Applies for SuperGrok
    /// live and console live (not SuperGrok-only).
    #[test]
    fn prepaid_lag_note_when_console_team_prepaid_dollars_shown() {
        let ttl = xai_grok_shell::auth::CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS;
        let expected = note_console_team_prepaid_may_lag();
        assert!(
            expected.contains(&format!("{ttl}s")),
            "note must cite TTL secs: {expected}"
        );
        assert!(
            expected.to_ascii_lowercase().contains("process cache"),
            "note must name process cache: {expected}"
        );
        assert!(
            expected.to_ascii_lowercase().contains("last successful"),
            "note must name app last-good that can outlive process TTL: {expected}"
        );
        assert!(
            expected.contains("grok limits"),
            "note must name CLI force path: {expected}"
        );
        assert!(
            expected.contains("/limits"),
            "note must name TUI force path: {expected}"
        );
        assert!(
            !expected.contains('\u{2014}') && !expected.contains("—"),
            "no em dash: {expected}"
        );

        let session = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::SuperGrokSession,
            has_included_reading: false,
            has_console_team_prepaid_reading: true,
            ..Default::default()
        });
        assert!(
            session
                .iter()
                .any(|n| n.as_str() == NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER),
            "license note first: {session:?}"
        );
        assert!(
            session.iter().any(|n| n == &expected),
            "prepaid lag on SuperGrok live: {session:?}"
        );

        let console = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::ConsoleKey,
            has_included_reading: true,
            flat_poll_unproven_debit: true,
            oauth_postpaid_dominates: true,
            has_console_team_prepaid_reading: true,
            ..Default::default()
        });
        assert!(
            console.iter().any(|n| n == &expected),
            "console live keeps prepaid lag note: {console:?}"
        );
        assert!(
            !console
                .iter()
                .any(|n| n.as_str() == NOTE_INCLUDED_PCT_IS_BILLING_POLL
                    || n.contains("included debit is unproven")
                    || n.as_str() == NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS),
            "console live: no SuperGrok burn notes: {console:?}"
        );
    }

    #[test]
    fn no_prepaid_lag_note_without_prepaid_dollars() {
        let notes = honesty_notes_for_limits(input_base());
        assert!(
            !notes
                .iter()
                .any(|n| n.contains("console team prepaid process cache may lag")
                    || n.contains("console team prepaid may lag")),
            "without prepaid $ reading must not claim lag: {notes:?}"
        );
    }

    /// Named contract: console live + flat flag still yields zero SuperGrok
    /// burn honesty notes (flat note says "session path can still be live").
    /// License-page meter honesty may still appear (not a SuperGrok burn claim).
    #[test]
    fn console_live_with_flat_flag_emits_no_supergrok_honesty() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::ConsoleKey,
            has_included_reading: true,
            flat_poll_unproven_debit: true,
            flat_poll_observed_build: true,
            flat_poll_observed_extras: true,
            oauth_postpaid_dominates: true,
            has_console_team_prepaid_reading: false,
            has_team_default_credits_reading: false,
        });
        assert!(
            !notes
                .iter()
                .any(|n| n.as_str() == NOTE_INCLUDED_PCT_IS_BILLING_POLL
                    || n.contains("included debit is unproven")
                    || n.as_str() == NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS),
            "console live + flat flag must not emit SuperGrok honesty: {notes:?}"
        );
        assert!(
            notes
                .iter()
                .any(|n| n.as_str() == NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER),
            "license page honesty still allowed: {notes:?}"
        );
    }

    #[test]
    fn no_base_note_without_included_reading() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::SuperGrokSession,
            has_included_reading: false,
            ..Default::default()
        });
        assert!(
            !notes
                .iter()
                .any(|n| n.as_str() == NOTE_INCLUDED_PCT_IS_BILLING_POLL),
            "no included meter → no poll note: {notes:?}"
        );
        assert!(
            notes
                .iter()
                .any(|n| n.as_str() == NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER),
            "license note still present without included: {notes:?}"
        );
    }

    #[test]
    fn flat_poll_note_when_evidence_flag_set() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            flat_poll_unproven_debit: true,
            flat_poll_observed_extras: true,
            ..input_base()
        });
        assert!(
            notes
                .iter()
                .any(|n| n.as_str() == NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER),
            "license note present: {notes:?}"
        );
        assert!(
            notes
                .iter()
                .any(|n| n.as_str() == NOTE_INCLUDED_PCT_IS_BILLING_POLL),
            "included poll note: {notes:?}"
        );
        let flat = notes
            .iter()
            .find(|n| n.contains("stayed flat"))
            .expect("flat note");
        assert!(
            flat.contains("included debit is unproven"),
            "flat note must say debit unproven: {flat}"
        );
        assert!(
            flat.contains("session path can still be live"),
            "flat note keeps session path honest: {flat}"
        );
        assert!(
            flat.contains("SuperGrok $ extras"),
            "extras observed flat must be named: {flat}"
        );
        assert!(
            !contains_forbidden_included_burn_claim(&notes.join("\n")),
            "flat notes must not overclaim burn"
        );
    }

    #[test]
    fn flat_note_alone_when_no_included_but_flag_set() {
        // Caller may surface flat-poll evidence even if snapshot rows are cold.
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::SuperGrokSession,
            has_included_reading: false,
            flat_poll_unproven_debit: true,
            flat_poll_observed_build: false,
            flat_poll_observed_extras: false,
            oauth_postpaid_dominates: false,
            has_console_team_prepaid_reading: false,
            has_team_default_credits_reading: false,
        });
        let expected_flat = flat_poll_unproven_debit_note(false, false);
        assert!(
            notes.iter().any(|n| n == &expected_flat),
            "flat-poll note when evidence set: {notes:?}"
        );
        assert!(
            notes
                .iter()
                .any(|n| n.as_str() == NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER),
            "license note still present: {notes:?}"
        );
    }

    /// Named contract (Issue 1): flat with Build never on wire must not claim
    /// Grok Build product % stayed flat.
    #[test]
    fn flat_poll_note_without_build_on_wire_does_not_claim_build_flat() {
        let note = flat_poll_unproven_debit_note(false, false);
        assert!(
            note.contains("SuperGrok included %"),
            "always names included: {note}"
        );
        assert!(
            !note.contains("Grok Build product"),
            "must not claim Build flat when never observed: {note}"
        );
        assert!(
            !note.contains("SuperGrok $ extras"),
            "must not claim extras flat when never observed: {note}"
        );
        assert!(
            note.contains("included debit is unproven"),
            "must still deny proven debit: {note}"
        );

        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            flat_poll_unproven_debit: true,
            flat_poll_observed_build: false,
            flat_poll_observed_extras: false,
            ..input_base()
        });
        let flat = notes
            .iter()
            .find(|n| n.contains("stayed flat"))
            .expect("flat");
        assert!(
            !flat.contains("Grok Build product"),
            "honesty stack must not overclaim Build: {flat}"
        );
    }

    /// Named contract: when Build and extras were observed flat, name them.
    #[test]
    fn flat_poll_note_names_build_and_extras_when_observed() {
        let note = flat_poll_unproven_debit_note(true, true);
        assert!(
            note.contains("Grok Build product %"),
            "must name Build when observed flat: {note}"
        );
        assert!(
            note.contains("SuperGrok $ extras"),
            "must name extras when observed flat: {note}"
        );
        assert!(
            note.contains("SuperGrok included %"),
            "must name included: {note}"
        );
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            flat_poll_unproven_debit: true,
            flat_poll_observed_build: true,
            flat_poll_observed_extras: true,
            ..input_base()
        });
        assert!(
            notes.iter().any(|n| n.contains("Grok Build product %")),
            "stack with observed Build: {notes:?}"
        );
    }

    /// Named contract C6: SuperGrok live + OAuth postpaid dominates → team Usage note.
    #[test]
    fn c6_team_usage_note_when_oauth_postpaid_dominates() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            oauth_postpaid_dominates: true,
            ..input_base()
        });
        assert!(
            notes
                .iter()
                .any(|n| n == NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS),
            "C6 note required when OAuth postpaid dominates: {notes:?}"
        );
        let c6 = NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS;
        assert!(
            c6.contains("team Usage dollars"),
            "must name team Usage dollars: {c6}"
        );
        assert!(
            c6.contains("OAuth") || c6.contains("Grok Build"),
            "must name OAuth / Grok Build class: {c6}"
        );
        assert!(
            c6.contains("console API key is not live"),
            "must say console key not live: {c6}"
        );
        assert!(
            c6.contains("without proving") && c6.contains("included weekly"),
            "branch 2b: OAuth Usage $ must not be sold as SuperGrok included debit: {c6}"
        );
        assert!(
            c6.contains("not SuperGrok dollar extras")
                || c6.contains("not SuperGrok dollar extras as the live driver"),
            "C6 must not sell settlement as SuperGrok extras: {c6}"
        );
        assert!(
            c6.contains("not free SuperGrok period burn proof"),
            "C6 must not sell settlement as free-period burn: {c6}"
        );
        assert!(
            !c6.contains('\u{2014}') && !c6.contains(" -- "),
            "no em dash in honesty copy: {c6}"
        );
        assert!(
            !contains_forbidden_included_burn_claim(c6),
            "C6 must not overclaim included burn"
        );
        // Without flat-poll evidence, settlement-rise strengthen note is off.
        assert!(
            !notes
                .iter()
                .any(|n| n == NOTE_FLAT_FREE_PERIOD_SETTLEMENT_RISE_NOT_EXTRAS),
            "flat+settlement note needs flat evidence: {notes:?}"
        );
    }

    /// SuperGrok live + flat (all meters) + C6: honesty notes ordered (license first).
    #[test]
    fn branch_2b_stack_base_flat_and_c6_when_evidence() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            flat_poll_unproven_debit: true,
            flat_poll_observed_build: true,
            flat_poll_observed_extras: true,
            oauth_postpaid_dominates: true,
            ..input_base()
        });
        assert_eq!(
            notes,
            vec![
                NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER.to_string(),
                NOTE_INCLUDED_PCT_IS_BILLING_POLL.to_string(),
                flat_poll_unproven_debit_note(true, true),
                NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS.to_string(),
                NOTE_FLAT_FREE_PERIOD_SETTLEMENT_RISE_NOT_EXTRAS.to_string(),
            ]
        );
        assert!(
            !contains_forbidden_included_burn_claim(&notes.join("\n")),
            "2b stack must not invent SuperGrok included burn"
        );
        let settle = NOTE_FLAT_FREE_PERIOD_SETTLEMENT_RISE_NOT_EXTRAS;
        assert!(
            settle.contains("team Grok Build")
                && (settle.contains("SuperGrok dollar extras")
                    || settle.contains("as SuperGrok dollar extras")),
            "settlement rise must name team class and reject SuperGrok extras label: {settle}"
        );
        assert!(
            settle.contains("does not treat team settlement as SuperGrok dollar extras")
                || settle.contains("does not treat team settlement"),
            "must not call team settlement SuperGrok extras: {settle}"
        );
        assert!(
            settle.contains("does not invent free-period debit"),
            "must keep C4 honesty (no invent debit): {settle}"
        );
    }

    #[test]
    fn no_c6_note_without_oauth_dominance() {
        let notes = honesty_notes_for_limits(input_base());
        assert!(
            !notes
                .iter()
                .any(|n| n == NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS),
            "must not invent C6 without postpaid evidence: {notes:?}"
        );
    }

    #[test]
    fn grok_build_product_usage_line_shared_phrase() {
        let line = format_grok_build_product_usage_line(54.7);
        assert_eq!(line, "Grok Build product usage: 54% used");
        assert!(
            line.ends_with("% used"),
            "shared phrase ends with % used: {line}"
        );
    }

    #[test]
    fn forbidden_claim_detector_catches_overclaims() {
        assert!(contains_forbidden_included_burn_claim(
            "We are using SuperGrok limits at 65%"
        ));
        assert!(contains_forbidden_included_burn_claim(
            "you are burning included limits now"
        ));
        // Case-insensitive: title-case / shouty overclaims still match.
        assert!(
            contains_forbidden_included_burn_claim("Using SuperGrok Limits at 65%"),
            "title-case overclaim must match"
        );
        assert!(
            contains_forbidden_included_burn_claim("USING SUPERGROK LIMITS"),
            "uppercase overclaim must match"
        );
        assert!(
            contains_forbidden_included_burn_claim("You Are Burning Included Limits"),
            "title-case burn claim must match"
        );
        assert!(!contains_forbidden_included_burn_claim(
            NOTE_INCLUDED_PCT_IS_BILLING_POLL
        ));
        assert!(!contains_forbidden_included_burn_claim(
            "Live sampling: SuperGrok session\nIncluded weekly allowance: 65% used"
        ));
        assert!(!contains_forbidden_included_burn_claim(
            &flat_poll_unproven_debit_note(false, false)
        ));
    }
}
