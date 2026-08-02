//! Honesty copy for SuperGrok limits surfaces (`/limits`, `grok limits`).
//!
//! Exact user-facing phrases live here so unit tests can assert them without
//! scattering string literals. Meters stay distinct: SuperGrok included weekly
//! % ≠ SuperGrok $ extras ≠ console team prepaid.
//!
//! **Named contracts (Slice 3):**
//! - Do not present SuperGrok included % as proven included-limit burn.
//! - Optional note when billing polls show flat included % and SuperGrok $
//!   extras (included debit unproven; session path can still be live).

use super::credit_bar::SamplingIdentityKind;

/// SuperGrok included % is a billing poll reading, not proof of burn.
///
/// Shown when live sampling is SuperGrok session and an included reading is
/// present. Plain American English; no em dash.
pub const NOTE_INCLUDED_PCT_IS_BILLING_POLL: &str = "Note: SuperGrok included % is the billing \
poll reading, not proof of included-limit burn.";

/// Polls kept the same included % and SuperGrok $ extras; debit unproven.
///
/// Optional. Only when the product has flat-poll evidence (not invented
/// inference counters). Session path can still be live.
pub const NOTE_FLAT_POLL_UNPROVEN_DEBIT: &str = "Note: included % and SuperGrok $ extras stayed \
flat across recent polls; included debit is unproven (session path can still be live).";

/// Phrases that must never appear as consumption claims from flat % alone.
pub const FORBIDDEN_INCLUDED_BURN_CLAIMS: &[&str] = &[
    "using SuperGrok limits",
    "burning included limits",
    "you are burning included",
];

/// Inputs for SuperGrok limits honesty notes (pure; hermetic tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LimitsHonestyInput {
    /// Live sampling identity (session path vs console key).
    pub live: SamplingIdentityKind,
    /// True when any SuperGrok principal row has an included % reading.
    pub has_included_reading: bool,
    /// True when product observed flat included % and SuperGrok $ extras
    /// across recent polls (caller-supplied evidence; do not invent).
    pub flat_poll_unproven_debit: bool,
}

/// Build honesty notes for limits modal / human `grok limits` (ordered).
///
/// - Base note when SuperGrok session is live and included % is shown.
/// - Flat-poll note only when SuperGrok session is live **and**
///   [`LimitsHonestyInput::flat_poll_unproven_debit`] (same gate as base; the
///   note's "session path can still be live" parenthetical is wrong under
///   console-live).
/// - Console-live: no SuperGrok burn / flat-poll honesty notes.
pub fn honesty_notes_for_limits(input: LimitsHonestyInput) -> Vec<&'static str> {
    let mut notes = Vec::new();
    if input.live.is_console() {
        return notes;
    }
    if input.has_included_reading {
        notes.push(NOTE_INCLUDED_PCT_IS_BILLING_POLL);
    }
    if input.flat_poll_unproven_debit {
        notes.push(NOTE_FLAT_POLL_UNPROVEN_DEBIT);
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

    #[test]
    fn base_note_when_supergrok_live_with_included_reading() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::SuperGrokSession,
            has_included_reading: true,
            flat_poll_unproven_debit: false,
        });
        assert_eq!(notes, vec![NOTE_INCLUDED_PCT_IS_BILLING_POLL]);
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
            !contains_forbidden_included_burn_claim(notes[0]),
            "honesty note must not overclaim: {}",
            notes[0]
        );
    }

    #[test]
    fn no_base_note_when_console_live() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::ConsoleKey,
            has_included_reading: true,
            flat_poll_unproven_debit: false,
        });
        assert!(
            notes.is_empty(),
            "console live must not sell SuperGrok burn notes: {notes:?}"
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
        });
        assert!(
            notes.is_empty(),
            "console live + flat flag must not emit SuperGrok honesty: {notes:?}"
        );
        assert!(
            !notes.contains(&NOTE_FLAT_POLL_UNPROVEN_DEBIT),
            "flat note gated on SuperGrok session, not console"
        );
    }

    #[test]
    fn no_base_note_without_included_reading() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::SuperGrokSession,
            has_included_reading: false,
            flat_poll_unproven_debit: false,
        });
        assert!(
            notes.is_empty(),
            "no included meter → no poll note: {notes:?}"
        );
    }

    #[test]
    fn flat_poll_note_when_evidence_flag_set() {
        let notes = honesty_notes_for_limits(LimitsHonestyInput {
            live: SamplingIdentityKind::SuperGrokSession,
            has_included_reading: true,
            flat_poll_unproven_debit: true,
        });
        assert_eq!(
            notes,
            vec![
                NOTE_INCLUDED_PCT_IS_BILLING_POLL,
                NOTE_FLAT_POLL_UNPROVEN_DEBIT
            ]
        );
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
        });
        assert_eq!(notes, vec![NOTE_FLAT_POLL_UNPROVEN_DEBIT]);
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
    }
}
