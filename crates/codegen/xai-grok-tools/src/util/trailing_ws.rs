//! Post-edit trailing-whitespace strip for product file-write tools.
//!
//! **Default ON.** After a successful text edit/write, strip spaces and tabs
//! at the end of each line before bytes hit disk. Does not change whether the
//! file ends with a newline (only per-line trailing spaces/tabs).
//!
//! ## Override
//!
//! Env `GROK_STRIP_TRAILING_WHITESPACE` (default on when unset):
//! - disable: `0`, `false`, `off`, `no`, `n` (case-insensitive)
//! - enable: `1`, `true`, `on`, `yes`, `y` (or any other non-empty value)
//!
//! Optional tool-level `Option<bool>` override (when a tool exposes it) wins
//! over env via [`should_strip`].
//!
//! Binary / non-text content is never stripped ([`crate::util::binary::is_binary`]).

use std::borrow::Cow;

use super::binary;

/// Env: control post-edit trailing-whitespace strip. Default ON when unset.
pub const ENV_STRIP_TRAILING_WHITESPACE: &str = "GROK_STRIP_TRAILING_WHITESPACE";

/// Whether strip is enabled from the environment.
///
/// Unset or empty → **true**. Explicit falsey tokens disable; truthy enable.
pub fn strip_enabled() -> bool {
    match std::env::var(ENV_STRIP_TRAILING_WHITESPACE) {
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

/// Resolve enablement: tool override (if any) wins, else env default.
pub fn should_strip(tool_override: Option<bool>) -> bool {
    tool_override.unwrap_or_else(strip_enabled)
}

/// Trim trailing spaces and tabs from a single line (no newline chars).
fn trim_eol_ws(line: &str) -> &str {
    line.trim_end_matches([' ', '\t'])
}

/// Strip trailing spaces/tabs at the end of each line.
///
/// - Preserves `\n` vs `\r\n` line endings
/// - Preserves whether the input ends with a newline
/// - Preserves blank lines (content becomes empty; ending kept)
/// - Does not touch spaces/tabs in the middle of a line
pub fn strip_trailing_whitespace(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while !rest.is_empty() {
        match rest.find('\n') {
            Some(pos) => {
                // Line includes everything up through `\n`.
                let line_end = pos + 1;
                let line_with_nl = &rest[..line_end];
                let (body, ending) = if let Some(body) = line_with_nl.strip_suffix("\r\n") {
                    (body, "\r\n")
                } else if let Some(body) = line_with_nl.strip_suffix('\n') {
                    (body, "\n")
                } else {
                    // Unreachable: match arm only when `\n` was found.
                    (line_with_nl, "")
                };
                out.push_str(trim_eol_ws(body));
                out.push_str(ending);
                rest = &rest[line_end..];
            }
            None => {
                // Final line without trailing newline.
                out.push_str(trim_eol_ws(rest));
                break;
            }
        }
    }

    out
}

/// Prepare text for a product file write: strip EOL spaces/tabs when enabled.
///
/// Skips when disabled or when content looks binary. Returns the original
/// string unchanged when strip would be a no-op.
pub fn prepare_for_write(text: String) -> String {
    prepare_for_write_with_override(text, None)
}

/// Like [`prepare_for_write`] with an optional tool-level override.
pub fn prepare_for_write_with_override(text: String, tool_override: Option<bool>) -> String {
    if !should_strip(tool_override) {
        return text;
    }
    if binary::is_binary("", text.as_bytes()) {
        return text;
    }
    let stripped = strip_trailing_whitespace(&text);
    if stripped == text { text } else { stripped }
}

/// Borrowing form of [`prepare_for_write`] for call sites that keep a source.
#[allow(dead_code)]
pub fn maybe_strip_text(text: &str, tool_override: Option<bool>) -> Cow<'_, str> {
    if !should_strip(tool_override) {
        return Cow::Borrowed(text);
    }
    if binary::is_binary("", text.as_bytes()) {
        return Cow::Borrowed(text);
    }
    let stripped = strip_trailing_whitespace(text);
    if stripped == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(stripped)
    }
}

/// Test helpers for process-global strip env mutation.
#[cfg(test)]
pub mod test_env {
    use std::sync::Mutex;

    /// Serialize env-mutating strip tests across the crate.
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

    #[test]
    fn strips_trailing_spaces_and_tabs() {
        assert_eq!(strip_trailing_whitespace("a  \nb\t\n"), "a\nb\n");
        assert_eq!(strip_trailing_whitespace("hello  "), "hello");
        assert_eq!(strip_trailing_whitespace("hello\t\t"), "hello");
    }

    #[test]
    fn preserves_internal_spaces() {
        assert_eq!(
            strip_trailing_whitespace("  indent keep  \n"),
            "  indent keep\n"
        );
        assert_eq!(strip_trailing_whitespace("a b c\n"), "a b c\n");
    }

    #[test]
    fn preserves_blank_lines() {
        assert_eq!(strip_trailing_whitespace("a\n\nb\n"), "a\n\nb\n");
        assert_eq!(strip_trailing_whitespace("a\n   \nb\n"), "a\n\nb\n");
    }

    #[test]
    fn preserves_crlf() {
        assert_eq!(strip_trailing_whitespace("a  \r\nb\t\r\n"), "a\r\nb\r\n");
        assert_eq!(strip_trailing_whitespace("only\r\n"), "only\r\n");
    }

    #[test]
    fn empty_file() {
        assert_eq!(strip_trailing_whitespace(""), "");
    }

    #[test]
    fn preserves_no_final_newline() {
        assert_eq!(strip_trailing_whitespace("a  \nb  "), "a\nb");
        assert_eq!(strip_trailing_whitespace("solo\t"), "solo");
    }

    #[test]
    fn preserves_final_newline_presence() {
        assert_eq!(strip_trailing_whitespace("a\n"), "a\n");
        assert_eq!(strip_trailing_whitespace("a  \n"), "a\n");
    }

    #[test]
    fn strip_enabled_default_on() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(&[(ENV_STRIP_TRAILING_WHITESPACE, None)]);
        assert!(strip_enabled());
        assert!(should_strip(None));
    }

    #[test]
    fn strip_enabled_false_tokens() {
        let _lock = ENV_LOCK.lock().unwrap();
        for token in ["0", "false", "FALSE", "off", "Off", "no", "n"] {
            let _env = EnvGuard::set(&[(ENV_STRIP_TRAILING_WHITESPACE, Some(token))]);
            assert!(!strip_enabled(), "token {token} should disable");
        }
    }

    #[test]
    fn strip_enabled_true_tokens() {
        let _lock = ENV_LOCK.lock().unwrap();
        for token in ["1", "true", "TRUE", "on", "yes", "y"] {
            let _env = EnvGuard::set(&[(ENV_STRIP_TRAILING_WHITESPACE, Some(token))]);
            assert!(strip_enabled(), "token {token} should enable");
        }
    }

    #[test]
    fn tool_override_wins() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(&[(ENV_STRIP_TRAILING_WHITESPACE, Some("0"))]);
        assert!(!strip_enabled());
        assert!(should_strip(Some(true)));
        assert!(!should_strip(Some(false)));

        let _env = EnvGuard::set(&[(ENV_STRIP_TRAILING_WHITESPACE, Some("1"))]);
        assert!(!should_strip(Some(false)));
        assert!(should_strip(Some(true)));
    }

    #[test]
    fn prepare_for_write_strips_when_on() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(&[(ENV_STRIP_TRAILING_WHITESPACE, Some("1"))]);
        let out = prepare_for_write("a  \nb\t\n".to_string());
        assert_eq!(out, "a\nb\n");
    }

    #[test]
    fn prepare_for_write_preserves_when_off() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(&[(ENV_STRIP_TRAILING_WHITESPACE, Some("0"))]);
        let src = "a  \nb\t\n".to_string();
        let out = prepare_for_write(src.clone());
        assert_eq!(out, src);
    }

    #[test]
    fn prepare_skips_binary_null_bytes() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(&[(ENV_STRIP_TRAILING_WHITESPACE, Some("1"))]);
        // Valid UTF-8 but null byte → binary; must not strip.
        let mut s = String::from("hello  ");
        s.push('\0');
        s.push_str("  ");
        let out = prepare_for_write(s.clone());
        assert_eq!(out, s, "binary content must not be stripped");
    }

    #[test]
    fn prepare_with_override_false_skips() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(&[(ENV_STRIP_TRAILING_WHITESPACE, Some("1"))]);
        let src = "a  \n".to_string();
        let out = prepare_for_write_with_override(src.clone(), Some(false));
        assert_eq!(out, src);
    }
}
