//! SHA-256 pin for published CLI artifacts. Not SHA-1.
//!
//! First field of the published `${artifact}.sha256` file must be exactly 64
//! hex digits (GNU `hash  name` or a bare hash). Fail-closed on a missing
//! file, an unreadable digest, or a mismatch. Do not log tokens.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Why a published SHA-256 pin refused the blob. Not SHA-1.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ChecksumVerifyFailure {
    #[error("SHA-256 checksum file missing ({url}). Refusing the download.")]
    Missing { url: String },
    #[error("checksum file is not a SHA-256 hex digest ({url}).")]
    Unreadable { url: String },
    #[error("SHA-256 mismatch for downloaded grok. Keeping the existing install.")]
    Mismatch,
    #[error("could not hash downloaded grok: {0}")]
    Io(String),
}

/// SHA-256 of `bytes` as lowercase hex. Not SHA-1.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Stream SHA-256 of a file. Does not load the whole file into memory.
pub fn sha256_hex_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// First field must be exactly 64 hex digits. Not SHA-1. Not the openssl
/// `SHA256 (name) = ...` tag form.
pub fn parse_sha256_file_bytes(bytes: &[u8]) -> Option<String> {
    let line = bytes.split(|b| *b == b'\n').next().unwrap_or(bytes);
    let line = match line.strip_suffix(b"\r") {
        Some(stripped) => stripped,
        None => line,
    };
    let first = line
        .split(|b| b.is_ascii_whitespace())
        .find(|part| !part.is_empty())?;
    let mut hex = String::from_utf8_lossy(first).into_owned();
    hex.make_ascii_lowercase();
    if let Some(stripped) = hex.strip_prefix('*') {
        hex = stripped.to_string();
    }
    if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(hex)
    } else {
        None
    }
}

/// Fail-closed compare of `path` against a 64-hex digest. Does not replace
/// any previous-good file; the caller must not publish until this returns
/// `Ok`.
pub fn verify_file_against_digest(
    path: &Path,
    expected: &str,
) -> Result<(), ChecksumVerifyFailure> {
    let actual = sha256_hex_file(path).map_err(|e| ChecksumVerifyFailure::Io(e.to_string()))?;
    if actual == expected {
        Ok(())
    } else {
        Err(ChecksumVerifyFailure::Mismatch)
    }
}

/// Published checksum URL for a successful artifact URL. Windows `.exe`
/// downloads use `${artifact}.exe.sha256`.
pub fn artifact_checksum_url(artifact_url: &str) -> String {
    format!("{artifact_url}.sha256")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Distinct from [`DOWNLOAD_BYTES`] so a refused verify cannot
    /// false-green by comparing identical previous-good and download bytes.
    const PREVIOUS_GOOD_BYTES: &[u8] = b"#!/bin/sh\nexit 0\n# prev\n";
    const DOWNLOAD_BYTES: &[u8] = b"#!/bin/sh\nexit 0\n";
    /// SHA-256 of [`DOWNLOAD_BYTES`]. Not SHA-1.
    const DOWNLOAD_SHA256: &str =
        "306c6ca7407560340797866e077e053627ad409277d1b9da58106fce4cf717cb";
    const WRONG_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn write_tmp(bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        f.write_all(bytes).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn parse_sha256_file_bytes_accepts_gnu_line_and_bare_hash() {
        assert_eq!(
            parse_sha256_file_bytes(format!("{DOWNLOAD_SHA256}  grok\n").as_bytes()).as_deref(),
            Some(DOWNLOAD_SHA256)
        );
        assert_eq!(
            parse_sha256_file_bytes(DOWNLOAD_SHA256.as_bytes()).as_deref(),
            Some(DOWNLOAD_SHA256)
        );
        assert_eq!(
            parse_sha256_file_bytes(format!("{DOWNLOAD_SHA256}\r\n").as_bytes()).as_deref(),
            Some(DOWNLOAD_SHA256)
        );
        let upper = DOWNLOAD_SHA256.to_ascii_uppercase();
        assert_eq!(
            parse_sha256_file_bytes(upper.as_bytes()).as_deref(),
            Some(DOWNLOAD_SHA256)
        );
        assert_eq!(
            parse_sha256_file_bytes(format!("*{DOWNLOAD_SHA256} grok.exe\n").as_bytes()).as_deref(),
            Some(DOWNLOAD_SHA256)
        );
    }

    #[test]
    fn parse_sha256_file_bytes_refuses_unreadable_and_sha1() {
        let bodies: &[&[u8]] = &[
            b"",
            b"not-a-digest\n",
            b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde\n",
            b"SHA256 (grok) = 306c6ca7407560340797866e077e053627ad409277d1b9da58106fce4cf717cb\n",
            b"gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg\n",
            // 40-hex SHA-1 is not a SHA-256 pin.
            b"0123456789abcdef0123456789abcdef01234567\n",
        ];
        for body in bodies {
            assert_eq!(
                parse_sha256_file_bytes(body),
                None,
                "must refuse unreadable checksum {body:?}"
            );
        }
    }

    #[test]
    fn sha256_hex_matches_known_download_bytes() {
        assert_eq!(sha256_hex(DOWNLOAD_BYTES), DOWNLOAD_SHA256);
        assert_ne!(sha256_hex(PREVIOUS_GOOD_BYTES), DOWNLOAD_SHA256);
        assert_ne!(PREVIOUS_GOOD_BYTES, DOWNLOAD_BYTES);
    }

    #[test]
    fn verify_file_against_digest_refuses_mismatch_and_keeps_previous_good() {
        let previous = write_tmp(PREVIOUS_GOOD_BYTES);
        let download = write_tmp(DOWNLOAD_BYTES);
        let err = verify_file_against_digest(download.path(), WRONG_SHA256).unwrap_err();
        assert_eq!(err, ChecksumVerifyFailure::Mismatch);
        assert_eq!(fs::read(previous.path()).unwrap(), PREVIOUS_GOOD_BYTES);
        assert_eq!(fs::read(download.path()).unwrap(), DOWNLOAD_BYTES);
        assert_ne!(fs::read(previous.path()).unwrap(), DOWNLOAD_BYTES);
    }

    #[test]
    fn verify_file_against_digest_accepts_matching_64_hex() {
        let download = write_tmp(DOWNLOAD_BYTES);
        verify_file_against_digest(download.path(), DOWNLOAD_SHA256).unwrap();
    }

    #[test]
    fn verify_file_against_digest_io_when_file_is_missing() {
        let missing = std::env::temp_dir().join("grok-sha256-missing-artifact-does-not-exist");
        let _ = fs::remove_file(&missing);
        let err = verify_file_against_digest(&missing, DOWNLOAD_SHA256).unwrap_err();
        assert!(matches!(err, ChecksumVerifyFailure::Io(_)));
    }

    #[test]
    fn artifact_checksum_url_appends_sha256_including_exe() {
        assert_eq!(
            artifact_checksum_url("https://x.ai/cli/grok-1.0.0-linux-x86_64"),
            "https://x.ai/cli/grok-1.0.0-linux-x86_64.sha256"
        );
        assert_eq!(
            artifact_checksum_url("https://x.ai/cli/grok-1.0.0-windows-x86_64.exe"),
            "https://x.ai/cli/grok-1.0.0-windows-x86_64.exe.sha256"
        );
        assert!(
            !artifact_checksum_url("https://x.ai/cli/grok").contains(".sha1"),
            "must not fetch SHA-1 checksums"
        );
    }
}
