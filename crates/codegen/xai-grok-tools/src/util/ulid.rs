//! ULID helpers for work / log / tool artifacts.
//!
//! Canonical form: **26-character Crockford base32** (no `I`/`L`/`O`/`U`),
//! time-sortable prefix (48-bit ms timestamp) + 80-bit randomness.
//!
//! Prefer this for **new** work/log/join/artifact ids. Do **not** mass-rewrite
//! existing task / tool-call UUID v7 sites — those stay UUID v7.

use std::path::Path;

use ulid::Ulid;

/// Length of a canonical ULID string (Crockford base32).
pub const ULID_STRING_LEN: usize = ulid::ULID_LEN;

/// Crockford base32 alphabet used by ULIDs (uppercase; excludes I, L, O, U).
pub const CROCKFORD_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Filename under a session directory holding the session-scoped work join key.
pub const WORK_ULID_FILE: &str = "work_ulid";

/// Mint a new ULID as a 26-character Crockford base32 string.
///
/// Suitable for work ids, log row join keys, and similar artifacts where
/// lexicographic order ≈ creation time is useful.
pub fn mint() -> String {
    Ulid::new().to_string()
}

/// Read a session work ULID from `{session_dir}/work_ulid` if valid.
pub fn read_work_ulid_file(session_dir: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(session_dir.join(WORK_ULID_FILE)).ok()?;
    let id = raw.trim();
    if is_valid(id) {
        Some(id.to_owned())
    } else {
        None
    }
}

/// Persist a session work ULID to `{session_dir}/work_ulid` (fail-open caller).
pub fn write_work_ulid_file(session_dir: &Path, work_ulid: &str) -> std::io::Result<()> {
    if let Some(parent) = session_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir_all(session_dir)?;
    std::fs::write(session_dir.join(WORK_ULID_FILE), work_ulid)
}

/// Parse a Crockford base32 ULID string.
///
/// Accepts the standard 26-character form (case-insensitive via the `ulid` crate).
pub fn parse(s: &str) -> Result<Ulid, ulid::DecodeError> {
    Ulid::from_string(s)
}

/// True when `s` is a valid canonical ULID string.
pub fn is_valid(s: &str) -> bool {
    parse(s).is_ok()
}

/// Round-trip: parse then re-encode. Useful for normalizing case.
pub fn normalize(s: &str) -> Result<String, ulid::DecodeError> {
    Ok(parse(s)?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn mint_length_is_26() {
        let id = mint();
        assert_eq!(id.len(), ULID_STRING_LEN);
        assert_eq!(id.len(), 26);
    }

    #[test]
    fn work_ulid_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let id = mint();
        write_work_ulid_file(dir.path(), &id).unwrap();
        assert_eq!(
            read_work_ulid_file(dir.path()).as_deref(),
            Some(id.as_str())
        );
        assert!(read_work_ulid_file(dir.path().join("missing").as_path()).is_none());
    }

    #[test]
    fn mint_charset_is_crockford_base32() {
        let id = mint();
        for (i, b) in id.bytes().enumerate() {
            assert!(
                CROCKFORD_ALPHABET.contains(&b),
                "char {i} = {b:?} ({}) not in Crockford alphabet; id={id}",
                b as char
            );
        }
        // Forbidden letters from base32 variants that Crockford omits
        for bad in b"ILOUilou" {
            assert!(!id.as_bytes().contains(bad), "forbidden char in {id}");
        }
    }

    #[test]
    fn mint_is_unique() {
        let mut seen = HashSet::new();
        for _ in 0..64 {
            assert!(seen.insert(mint()), "duplicate ULID in small sample");
        }
    }

    #[test]
    fn mint_timestamp_prefix_is_non_decreasing() {
        // With a short sleep, ms timestamps advance so string order tracks time.
        let a = mint();
        thread::sleep(Duration::from_millis(2));
        let b = mint();
        assert!(
            a.as_str() < b.as_str(),
            "expected later ULID to sort after earlier: a={a} b={b}"
        );
        // Same-ms batch: still valid ULIDs (order among equals is random-bit dependent)
        let batch: Vec<String> = (0..8).map(|_| mint()).collect();
        for id in &batch {
            assert!(is_valid(id));
            assert_eq!(id.len(), 26);
        }
    }

    #[test]
    fn parse_roundtrip() {
        let id = mint();
        let parsed = parse(&id).expect("parse mint output");
        assert_eq!(parsed.to_string(), id);
        assert_eq!(normalize(&id).unwrap(), id);
    }

    #[test]
    fn parse_accepts_lowercase() {
        let id = mint();
        let lower = id.to_ascii_lowercase();
        let normalized = normalize(&lower).expect("lowercase ULID");
        assert_eq!(normalized, id);
    }

    #[test]
    fn parse_rejects_bad_input() {
        assert!(parse("").is_err());
        assert!(parse("too-short").is_err());
        assert!(parse("01ARZ3NDEKTSV4RRFFQ69G5FAV!").is_err()); // bad char
        assert!(parse(&"0".repeat(26)).is_ok()); // all zeros is valid ULID
        assert!(!is_valid("not-a-ulid"));
        assert!(!is_valid(&"Z".repeat(25))); // wrong length
    }

    #[test]
    fn known_vector_roundtrip() {
        // Spec / community fixture (ulid-rs README style)
        let s = "01D39ZY06FGSCTVN4T2V9PKHFZ";
        let u = parse(s).expect("known vector");
        assert_eq!(u.to_string(), s);
        assert!(is_valid(s));
    }
}
