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
/// remaining, and from SuperGrok included weekly debit proof.
pub const NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS: &str = "Note: SuperGrok session can still \
move team Usage dollars (OAuth / Grok Build class on the team invoice) without proving \
SuperGrok included weekly moved, even when the console API key is not live.";

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

/// Shared Grok Build productUsage line (human `/limits` and `/usage`).
///
/// Floors like included %. Always ends with `% used` so surfaces match.
/// Distinct from top-level included allowance %. Never invent when absent.
pub fn format_grok_build_product_usage_line(pct: f64) -> String {
    format!("Grok Build product usage: {}% used", pct.floor() as i64)
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
/// - Prepaid lag note when console team prepaid dollars are shown (any live
///   identity). Names process-cache lag + `grok limits` / `/limits` force-refresh.
/// - Base note when SuperGrok session is live and included % is shown.
/// - Flat-poll note only when SuperGrok session is live **and**
///   [`LimitsHonestyInput::flat_poll_unproven_debit`]. Meter names come from
///   observed flags (no invent Build/extras flat claim).
/// - C6 team Usage note when SuperGrok session is live **and**
///   [`LimitsHonestyInput::oauth_postpaid_dominates`].
/// - Console-live: no SuperGrok burn / flat-poll / C6 honesty notes (prepaid
///   lag note still allowed when dollars are shown).
pub fn honesty_notes_for_limits(input: LimitsHonestyInput) -> Vec<String> {
    let mut notes = Vec::new();
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
        assert_eq!(notes, vec![NOTE_INCLUDED_PCT_IS_BILLING_POLL.to_string()]);
        assert!(
            notes[0].contains("billing poll reading"),
            "must name poll reading: {}",
            notes[0]
        );
        assert!(
            notes[0].contains("not proof of included-limit burn"),
            "must deny burn proof: {}",
            notes[0]
        );
        assert!(
            !contains_forbidden_included_burn_claim(&notes[0]),
            "honesty note must not overclaim: {}",
            notes[0]
        );
    }

    #[test]
    fn no_base_note_when_console_live() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::ConsoleKey,
            has_included_reading: true,
            ..Default::default()
        });
        assert!(
            notes.is_empty(),
            "console live must not sell SuperGrok burn notes: {notes:?}"
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
        assert_eq!(session, vec![expected.clone()]);

        let console = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::ConsoleKey,
            has_included_reading: true,
            flat_poll_unproven_debit: true,
            oauth_postpaid_dominates: true,
            has_console_team_prepaid_reading: true,
            ..Default::default()
        });
        assert_eq!(
            console,
            vec![expected],
            "console live keeps prepaid lag note only (no SuperGrok burn notes)"
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
    /// honesty notes (flat note says "session path can still be live").
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
            notes.is_empty(),
            "console live + flat flag must not emit SuperGrok honesty: {notes:?}"
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
            notes.is_empty(),
            "no included meter → no poll note: {notes:?}"
        );
    }

    #[test]
    fn flat_poll_note_when_evidence_flag_set() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            flat_poll_unproven_debit: true,
            flat_poll_observed_extras: true,
            ..input_base()
        });
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0], NOTE_INCLUDED_PCT_IS_BILLING_POLL);
        assert!(
            notes[1].contains("stayed flat"),
            "flat note must say meters stayed flat: {}",
            notes[1]
        );
        assert!(
            notes[1].contains("included debit is unproven"),
            "flat note must say debit unproven: {}",
            notes[1]
        );
        assert!(
            notes[1].contains("session path can still be live"),
            "flat note keeps session path honest: {}",
            notes[1]
        );
        assert!(
            notes[1].contains("SuperGrok $ extras"),
            "extras observed flat must be named: {}",
            notes[1]
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
        assert_eq!(notes, vec![flat_poll_unproven_debit_note(false, false)]);
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
            !c6.contains('\u{2014}') && !c6.contains(" -- "),
            "no em dash in honesty copy: {c6}"
        );
        assert!(
            !contains_forbidden_included_burn_claim(c6),
            "C6 must not overclaim included burn"
        );
    }

    /// SuperGrok live + flat (all meters) + C6: three honesty notes, ordered.
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
                NOTE_INCLUDED_PCT_IS_BILLING_POLL.to_string(),
                flat_poll_unproven_debit_note(true, true),
                NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS.to_string(),
            ]
        );
        assert!(
            !contains_forbidden_included_burn_claim(&notes.join("\n")),
            "2b stack must not invent SuperGrok included burn"
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
