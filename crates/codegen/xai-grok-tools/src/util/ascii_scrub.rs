//! ASCII scrub of model prose for ASCII-first terminals and workflows.
//!
//! Pure map in [`scrub_ascii_punct`] / [`needs_ascii_scrub`]. Enablement
//! (default **ON**) via env kill-switch [`ENV_SCRUB_ASCII_PUNCT`] and optional
//! override ([`should_scrub`] / [`maybe_scrub_ascii_punct_owned`]). Stream /
//! chat_state wiring lives in the shell (S1); durable `[ui]` key and
//! approval override are S2/S3.
//!
//! ## Replacements
//!
//! | Source | Result |
//! |--------|--------|
//! | Em dash U+2014 | `--` |
//! | En dash U+2013 | `-` |
//! | Smart double quotes U+201C U+201D | `"` |
//! | Smart apostrophes / single quotes U+2018 U+2019 | `'` |
//! | Zero-width / invisible format chars (see below) | stripped (empty) |
//! | Non-breaking / exotic spaces (see below) | ASCII space ` ` |
//!
//! ### Zero-width → empty (strip)
//!
//! These are invisible and almost always accidental in model prose:
//!
//! - U+200B zero-width space (ZWSP)
//! - U+200C zero-width non-joiner (ZWNJ)
//! - U+200D zero-width joiner (ZWJ)
//! - U+2060 word joiner
//! - U+FEFF BOM / zero-width no-break space
//!
//! ### Space-like → regular space
//!
//! Visible width or line-breaking variants that should still separate tokens:
//!
//! - U+00A0 no-break space (NBSP)
//! - U+202F narrow no-break space
//! - U+2007 figure space
//! - U+2008 punctuation space
//! - U+2009 thin space
//! - U+200A hair space
//! - U+205F medium mathematical space
//!
//! Characters not listed above (emoji, CJK, intentional non-ASCII) pass through
//! unchanged.
//!
//! ## Enablement (default ON)
//!
//! Env [`ENV_SCRUB_ASCII_PUNCT`] (`GROK_SCRUB_ASCII_PUNCT`), default on when
//! unset (same tokens as trailing-ws strip):
//! - disable: `0`, `false`, `off`, `no`, `n` (case-insensitive)
//! - enable: `1`, `true`, `on`, `yes`, `y` (or any other non-empty value)
//!
//! Optional `Option<bool>` override (config / session) wins via
//! [`should_scrub`].
//!
//! ## Relation to [`super::unicode_confusables`]
//!
//! That module normalizes typography for **file match** fallbacks
//! (`search_replace`). This module is the **assistant-output** scrub choke
//! helper. Maps overlap on dashes/quotes/NBSP but differ on zero-width strip
//! and product intent — keep them separate.

use std::borrow::Cow;

/// Env: control assistant-text ASCII scrub. Default ON when unset.
pub const ENV_SCRUB_ASCII_PUNCT: &str = "GROK_SCRUB_ASCII_PUNCT";

/// Whether scrub is enabled from the environment.
///
/// Unset or empty → **true**. Explicit falsey tokens disable; truthy enable.
pub fn scrub_enabled() -> bool {
    match std::env::var(ENV_SCRUB_ASCII_PUNCT) {
        Err(_) => true,
        Ok(v) => {
            let v = v.trim();
            if v.is_empty() {
                return true;
            }
            match v.to_ascii_lowercase().as_str() {
                "0" | "false" | "off" | "no" | "n" => false,
                "1" | "true" | "on" | "yes" | "y" => true,
                // Unknown non-empty values: treat as on (hygiene default).
                _ => true,
            }
        }
    }
}

/// Resolve enablement: override (config / session) wins when `Some`, else env.
pub fn should_scrub(override_: Option<bool>) -> bool {
    override_.unwrap_or_else(scrub_enabled)
}

/// Whether `c` has a scrub replacement (including strip-to-empty).
fn map_char(c: char) -> Option<&'static str> {
    match c {
        // Dashes
        '\u{2014}' => Some("--"), // em dash
        '\u{2013}' => Some("-"),  // en dash
        // Smart double quotes
        '\u{201C}' | '\u{201D}' => Some("\""),
        // Smart apostrophes / single quotes
        '\u{2018}' | '\u{2019}' => Some("'"),
        // Zero-width / invisible → strip
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}' => Some(""),
        // Space-like → ASCII space
        '\u{00A0}' | '\u{202F}' | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}'
        | '\u{205F}' => Some(" "),
        _ => None,
    }
}

/// Fast check: does `s` contain any character this scrub rewrites?
pub fn needs_ascii_scrub(s: &str) -> bool {
    s.chars().any(|c| map_char(c).is_some())
}

/// Map AI/model prose punctuation and invisible spaces to ASCII-safe forms.
///
/// See module docs for the exact character classes. Pure: no env, no config.
pub fn scrub_ascii_punct(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    // Capacity: em dash can shrink (3 UTF-8 bytes → 2); most cases stay ≤ len.
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match map_char(c) {
            Some(replacement) => out.push_str(replacement),
            None => out.push(c),
        }
    }
    out
}

/// Apply scrub when enabled (env / override); otherwise return `text` unchanged.
pub fn maybe_scrub_ascii_punct<'a>(text: &'a str, override_: Option<bool>) -> Cow<'a, str> {
    if !should_scrub(override_) {
        return Cow::Borrowed(text);
    }
    if !needs_ascii_scrub(text) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(scrub_ascii_punct(text))
}

/// Owned form for stream/chat_state call sites that already hold a `String`.
pub fn maybe_scrub_ascii_punct_owned(text: String, override_: Option<bool>) -> String {
    if !should_scrub(override_) {
        return text;
    }
    if !needs_ascii_scrub(&text) {
        return text;
    }
    scrub_ascii_punct(&text)
}

/// Test helpers for process-global scrub env mutation.
#[cfg(test)]
pub mod test_env {
    use std::sync::Mutex;

    /// Serialize env-mutating scrub tests across the crate.
    pub static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Sets env pairs; removes those keys on drop (panic-safe cleanup).
    pub struct EnvGuard {
        keys: Vec<&'static str>,
    }

    impl EnvGuard {
        pub fn set(pairs: &[(&'static str, Option<&str>)]) -> Self {
            let keys: Vec<_> = pairs.iter().map(|(k, _)| *k).collect();
            for (k, v) in pairs {
                match v {
                    Some(val) => unsafe { std::env::set_var(k, val) },
                    None => unsafe { std::env::remove_var(k) },
                }
            }
            Self { keys }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for k in &self.keys {
                unsafe { std::env::remove_var(k) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_env::{ENV_LOCK, EnvGuard};
    use super::*;

    // ── Dashes ──────────────────────────────────────────────────────────

    #[test]
    fn em_dash_to_double_hyphen() {
        assert_eq!(scrub_ascii_punct("foo\u{2014}bar"), "foo--bar");
        assert_eq!(scrub_ascii_punct("\u{2014}"), "--");
    }

    #[test]
    fn en_dash_to_hyphen() {
        assert_eq!(scrub_ascii_punct("10\u{2013}20"), "10-20");
        assert_eq!(scrub_ascii_punct("\u{2013}"), "-");
    }

    // ── Smart quotes ────────────────────────────────────────────────────

    #[test]
    fn smart_double_quotes_to_ascii() {
        assert_eq!(scrub_ascii_punct("\u{201C}hello\u{201D}"), "\"hello\"");
        assert_eq!(scrub_ascii_punct("say \u{201C}hi\u{201D}"), "say \"hi\"");
    }

    #[test]
    fn smart_apostrophes_to_ascii() {
        assert_eq!(scrub_ascii_punct("it\u{2019}s"), "it's");
        assert_eq!(scrub_ascii_punct("\u{2018}quoted\u{2019}"), "'quoted'");
    }

    // ── Zero-width → empty ──────────────────────────────────────────────

    #[test]
    fn zero_width_space_stripped() {
        assert_eq!(scrub_ascii_punct("a\u{200B}b"), "ab");
        assert_eq!(scrub_ascii_punct("\u{200B}"), "");
    }

    #[test]
    fn zero_width_joiners_stripped() {
        assert_eq!(scrub_ascii_punct("a\u{200C}b\u{200D}c"), "abc");
        assert_eq!(scrub_ascii_punct("x\u{2060}y"), "xy");
    }

    #[test]
    fn bom_zwnbsp_stripped() {
        assert_eq!(scrub_ascii_punct("\u{FEFF}hello\u{FEFF}"), "hello");
    }

    // ── Space-like → regular space ──────────────────────────────────────

    #[test]
    fn nbsp_to_space() {
        assert_eq!(scrub_ascii_punct("hello\u{00A0}world"), "hello world");
    }

    #[test]
    fn narrow_nbsp_and_thin_spaces_to_space() {
        assert_eq!(scrub_ascii_punct("a\u{202F}b"), "a b");
        assert_eq!(scrub_ascii_punct("a\u{2009}b"), "a b");
        assert_eq!(scrub_ascii_punct("a\u{200A}b"), "a b");
        assert_eq!(scrub_ascii_punct("a\u{2007}b"), "a b");
        assert_eq!(scrub_ascii_punct("a\u{2008}b"), "a b");
        assert_eq!(scrub_ascii_punct("a\u{205F}b"), "a b");
    }

    // ── Identity / passthrough ──────────────────────────────────────────

    #[test]
    fn pure_ascii_unchanged() {
        let s = "The quick brown fox - wait, no fancy chars. 0123 !@#";
        assert_eq!(scrub_ascii_punct(s), s);
        assert!(!needs_ascii_scrub(s));
    }

    #[test]
    fn empty_string() {
        assert_eq!(scrub_ascii_punct(""), "");
        assert!(!needs_ascii_scrub(""));
    }

    #[test]
    fn preserves_non_target_unicode() {
        let s = "hello 🌍 世界 café";
        assert_eq!(scrub_ascii_punct(s), s);
        assert!(!needs_ascii_scrub(s));
    }

    #[test]
    fn preserves_ascii_quotes_and_hyphens() {
        assert_eq!(
            scrub_ascii_punct("\"hello\" -- it's-fine"),
            "\"hello\" -- it's-fine"
        );
    }

    // ── Compound ────────────────────────────────────────────────────────

    #[test]
    fn mixed_typography_line() {
        let s = "She said \u{201C}go\u{201D}\u{00A0}\u{2014}\u{00A0}now\u{2019}s fine";
        assert_eq!(scrub_ascii_punct(s), "She said \"go\" -- now's fine");
    }

    #[test]
    fn zero_width_inside_word_and_quotes() {
        let s = "\u{201C}invis\u{200B}ible\u{201D}";
        assert_eq!(scrub_ascii_punct(s), "\"invisible\"");
    }

    #[test]
    fn needs_ascii_scrub_true_for_each_class() {
        assert!(needs_ascii_scrub("a\u{2014}b"));
        assert!(needs_ascii_scrub("a\u{2013}b"));
        assert!(needs_ascii_scrub("\u{201C}x\u{201D}"));
        assert!(needs_ascii_scrub("it\u{2019}s"));
        assert!(needs_ascii_scrub("a\u{200B}b"));
        assert!(needs_ascii_scrub("a\u{00A0}b"));
    }

    #[test]
    fn idempotent_on_scrubbed_output() {
        let s = "\u{201C}a\u{2014}b\u{201D}\u{00A0}c\u{200B}d";
        let once = scrub_ascii_punct(s);
        let twice = scrub_ascii_punct(&once);
        assert_eq!(once, twice);
        assert!(!needs_ascii_scrub(&once));
    }

    // ── Enablement (S2) ─────────────────────────────────────────────────

    #[test]
    fn scrub_enabled_default_on() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(&[(ENV_SCRUB_ASCII_PUNCT, None)]);
        assert!(scrub_enabled());
        assert!(should_scrub(None));
    }

    #[test]
    fn scrub_enabled_false_tokens() {
        let _lock = ENV_LOCK.lock().unwrap();
        for token in ["0", "false", "FALSE", "off", "Off", "no", "n"] {
            let _env = EnvGuard::set(&[(ENV_SCRUB_ASCII_PUNCT, Some(token))]);
            assert!(!scrub_enabled(), "token {token} should disable");
        }
    }

    #[test]
    fn scrub_enabled_true_tokens() {
        let _lock = ENV_LOCK.lock().unwrap();
        for token in ["1", "true", "TRUE", "on", "yes", "y"] {
            let _env = EnvGuard::set(&[(ENV_SCRUB_ASCII_PUNCT, Some(token))]);
            assert!(scrub_enabled(), "token {token} should enable");
        }
    }

    #[test]
    fn override_wins_over_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(&[(ENV_SCRUB_ASCII_PUNCT, Some("0"))]);
        assert!(!scrub_enabled());
        assert!(should_scrub(Some(true)));
        assert!(!should_scrub(Some(false)));

        let _env = EnvGuard::set(&[(ENV_SCRUB_ASCII_PUNCT, Some("1"))]);
        assert!(!should_scrub(Some(false)));
        assert!(should_scrub(Some(true)));
    }

    #[test]
    fn maybe_scrub_applies_when_on() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(&[(ENV_SCRUB_ASCII_PUNCT, Some("1"))]);
        let raw = "say \u{201C}hi\u{201D}\u{2014}now";
        assert_eq!(
            maybe_scrub_ascii_punct(raw, None).as_ref(),
            "say \"hi\"--now"
        );
        assert_eq!(
            maybe_scrub_ascii_punct_owned(raw.to_string(), None),
            "say \"hi\"--now"
        );
    }

    #[test]
    fn maybe_scrub_preserves_when_off() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(&[(ENV_SCRUB_ASCII_PUNCT, Some("0"))]);
        let raw = "say \u{201C}hi\u{201D}\u{2014}now";
        assert_eq!(maybe_scrub_ascii_punct(raw, None).as_ref(), raw);
        assert_eq!(maybe_scrub_ascii_punct_owned(raw.to_string(), None), raw);
    }

    #[test]
    fn maybe_scrub_override_false_preserves_despite_env_on() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(&[(ENV_SCRUB_ASCII_PUNCT, Some("1"))]);
        let raw = "it\u{2019}s";
        assert_eq!(
            maybe_scrub_ascii_punct_owned(raw.to_string(), Some(false)),
            raw
        );
    }
}
