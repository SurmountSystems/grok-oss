//! Contract tests for the published Windows installers (`install.ps1`,
//! `install-enterprise.ps1`). Bootstrap must work without Nix: the scripts
//! pin SHA-256 with built-in `Get-FileHash`, not a compiled helper.
//!
//! Missing scripts fail the pin (do not skip-pass). Runtime `pwsh` is
//! optional extra coverage for parse refuse and Verify throw.

use std::path::{Path, PathBuf};
use std::process::Command;

fn script_path(name: &str) -> PathBuf {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../xai-grok-pager/scripts/{name}"));
    dunce::canonicalize(&path).unwrap_or_else(|e| {
        panic!(
            "{name} must exist relative to crate (bad CARGO_MANIFEST_DIR cannot green the pin): {} ({e})",
            path.display()
        )
    })
}

const WINDOWS_INSTALL_SCRIPTS: [&str; 2] = ["install.ps1", "install-enterprise.ps1"];

fn required_script_body(name: &str) -> String {
    let path = script_path(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("read {}: {e}", path.display());
    })
}

fn extract_function<'a>(body: &'a str, name: &str) -> &'a str {
    let needle = format!("function {name}");
    let start = body
        .find(&needle)
        .unwrap_or_else(|| panic!("missing {needle}"));
    let rest = &body[start + needle.len()..];
    let end_rel = rest.find("\nfunction ").unwrap_or(rest.len());
    &body[start..start + needle.len() + end_rel]
}

const UNREADABLE_CHECKSUM_BODIES: &[&str] = &[
    "",
    "not-a-digest\n",
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde\n",
    "SHA256 (grok) = 306c6ca7407560340797866e077e053627ad409277d1b9da58106fce4cf717cb\n",
    "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg\n",
    "0123456789abcdef0123456789abcdef01234567\n",
];

const DOWNLOAD_SHA256: &str = "306c6ca7407560340797866e077e053627ad409277d1b9da58106fce4cf717cb";
const PREVIOUS_GOOD: &str = "previous-good-bytes\n";
const DOWNLOAD_PAYLOAD: &str = "download-payload-bytes\n";

fn pwsh_program() -> Option<&'static str> {
    for name in ["pwsh", "powershell"] {
        let ok = Command::new(name)
            .arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-Command")
            .arg("exit 0")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some(name);
        }
    }
    None
}

/// Named contract: both published PowerShell installers fetch
/// `${artifact}.sha256`, hash with SHA-256 (not SHA-1), require 64 hex,
/// refuse before replacing the installed copy, and abort on pin miss.
#[test]
fn windows_install_scripts_pin_published_sha256_not_sha1() {
    for script in WINDOWS_INSTALL_SCRIPTS {
        let body = required_script_body(script);
        assert!(
            body.contains(".sha256"),
            "{script} must download a published SHA-256 checksum file"
        );
        assert!(
            !body.contains(".sha1"),
            "{script} must not fetch SHA-1 checksums"
        );
        assert!(
            body.contains("Get-FileHash") && body.contains("SHA256"),
            "{script} must hash the download with Get-FileHash SHA256 (built-in, no Nix)"
        );
        assert!(
            !body.contains("-Algorithm SHA1") && !body.contains("-Algorithm Sha1"),
            "{script} must not use SHA-1 as the download pin"
        );
        assert!(
            body.contains("^[0-9a-f]{64}$") || body.contains("^[0-9a-fA-F]{64}$"),
            "{script} must require a 64-hex digest"
        );
        assert!(
            body.contains("checksum file missing")
                || body.contains("SHA-256 checksum file missing"),
            "{script} must refuse when the published checksum file is missing"
        );
        assert!(
            body.contains("not a SHA-256 hex digest"),
            "{script} must refuse an unreadable checksum file"
        );
        assert!(
            body.contains("SHA-256 mismatch"),
            "{script} must refuse when the digest does not match"
        );
        assert!(
            body.contains("Keeping the existing install"),
            "{script} must keep previous-good on mismatch"
        );
        let verify_at = body
            .find("Verify-DownloadedSha256 $binaryTmp")
            .unwrap_or_else(|| {
                panic!("{script} must call Verify-DownloadedSha256 $binaryTmp (call site, not the function definition)")
            });
        let copy_at = body
            .find("Copy-Item -Path $binaryPath")
            .unwrap_or_else(|| panic!("{script} must install with Copy-Item after verify"));
        assert!(
            verify_at < copy_at,
            "{script} must verify SHA-256 before Copy-Item replaces previous-good"
        );
        let move_at = body
            .find("Move-Item -LiteralPath $binaryTmp")
            .unwrap_or_else(|| panic!("{script} must Move-Item the verified temp onto the cache"));
        assert!(
            verify_at < move_at,
            "{script} must verify SHA-256 before Move-Item replaces the download cache"
        );
        assert!(
            body.contains(".tmp.$PID"),
            "{script} must use a unique pending path (PID), not a shared .tmp"
        );
        let verify_fn = extract_function(&body, "Verify-DownloadedSha256");
        assert!(
            verify_fn.contains("throw \"SHA-256 checksum file missing")
                || verify_fn.contains("throw 'SHA-256 checksum file missing"),
            "{script} Verify-DownloadedSha256 must throw on a missing checksum file"
        );
        assert!(
            verify_fn.contains("throw \"checksum file is not a SHA-256 hex digest")
                || verify_fn.contains("throw 'checksum file is not a SHA-256 hex digest"),
            "{script} Verify-DownloadedSha256 must throw on an unreadable checksum file"
        );
        assert!(
            verify_fn.contains("throw \"SHA-256 mismatch")
                || verify_fn.contains("throw 'SHA-256 mismatch"),
            "{script} Verify-DownloadedSha256 must throw on SHA-256 mismatch"
        );
        assert!(
            !verify_fn
                .lines()
                .any(|l| matches!(l.trim(), "return" | "return $false" | "return $null")),
            "{script} Verify-DownloadedSha256 must not return on a pin refuse (that would fall through to Move-Item / Copy-Item)"
        );
    }
}

/// Same fail-closed parse contract as POSIX `install.sh` / the Rust helper.
/// PowerShell bootstrap cannot call that helper (no Nix, no grok yet).
#[test]
fn windows_install_scripts_parse_contract_matches_rust_helper() {
    for script in WINDOWS_INSTALL_SCRIPTS {
        let body = required_script_body(script);
        let parse_fn = extract_function(&body, "Parse-Sha256File");
        assert!(
            parse_fn.contains("Parse-Sha256File"),
            "{script} must parse the published checksum file"
        );
        let gnu_ok = xai_grok_update::artifact_sha256::parse_sha256_file_bytes(
            format!("{DOWNLOAD_SHA256}  grok\n").as_bytes(),
        );
        assert!(gnu_ok.is_some());
        for unreadable in UNREADABLE_CHECKSUM_BODIES {
            assert!(
                xai_grok_update::artifact_sha256::parse_sha256_file_bytes(unreadable.as_bytes())
                    .is_none(),
                "Rust helper must refuse unreadable checksum {unreadable:?}"
            );
        }
        if let Some(pwsh) = pwsh_program() {
            run_pwsh_parse_refuse(pwsh, parse_fn, script);
        } else {
            eprintln!("pwsh not on PATH; source contract still required, runtime parse skipped");
        }
    }
}

fn run_pwsh_parse_refuse(pwsh: &str, parse_fn: &str, script: &str) {
    let dir = tempfile::tempdir().unwrap();
    for (i, body) in UNREADABLE_CHECKSUM_BODIES.iter().enumerate() {
        std::fs::write(dir.path().join(format!("bad{i}.sha256")), body.as_bytes()).unwrap();
    }
    std::fs::write(
        dir.path().join("ok.sha256"),
        format!("{DOWNLOAD_SHA256}  grok\n"),
    )
    .unwrap();
    let harness = dir.path().join("parse.ps1");
    let dir_ps = dir.path().display().to_string().replace('\'', "''");
    let ps = format!(
        r#"
$ErrorActionPreference = 'Stop'
{parse_fn}
$dir = '{dir_ps}'
Get-ChildItem -LiteralPath $dir -Filter 'bad*.sha256' | ForEach-Object {{
  $got = Parse-Sha256File $_.FullName
  if ($null -ne $got -and "$got" -ne '') {{
    throw "Parse-Sha256File accepted unreadable checksum $($_.Name)"
  }}
}}
$ok = Parse-Sha256File (Join-Path $dir 'ok.sha256')
if ($ok -ne '{DOWNLOAD_SHA256}') {{
  throw "Parse-Sha256File rejected a GNU SHA-256 line"
}}
exit 0
"#
    );
    std::fs::write(&harness, ps).unwrap();
    let status = Command::new(pwsh)
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-File")
        .arg(&harness)
        .status()
        .unwrap_or_else(|e| panic!("spawn {pwsh}: {e}"));
    assert!(
        status.success(),
        "{script}: Parse-Sha256File must refuse the same unreadable bodies as the Rust helper"
    );
}

/// When pwsh is present: Verify-DownloadedSha256 must throw on mismatch,
/// missing checksum, and unreadable checksum. Previous-good dest bytes stay
/// distinct from the download. Copy-Item must not run.
#[test]
fn windows_verify_throws_and_keeps_previous_good_when_pwsh_present() {
    let Some(pwsh) = pwsh_program() else {
        eprintln!("pwsh not on PATH; runtime Verify-DownloadedSha256 fixture skipped");
        return;
    };
    for script in WINDOWS_INSTALL_SCRIPTS {
        let body = required_script_body(script);
        let get_hash = extract_function(&body, "Get-FileSha256");
        let parse_fn = extract_function(&body, "Parse-Sha256File");
        let verify_fn = extract_function(&body, "Verify-DownloadedSha256");
        run_pwsh_verify_fixture(pwsh, script, get_hash, parse_fn, verify_fn);
    }
}

fn run_pwsh_verify_fixture(
    pwsh: &str,
    script: &str,
    get_hash: &str,
    parse_fn: &str,
    verify_fn: &str,
) {
    assert_ne!(PREVIOUS_GOOD, DOWNLOAD_PAYLOAD);
    let cases = [
        (
            "mismatch",
            "0000000000000000000000000000000000000000000000000000000000000000  grok\n",
        ),
        ("unreadable", "not-a-digest\n"),
        ("missing", ""),
    ];
    for (mode, checksum_body) in cases {
        let dir = tempfile::tempdir().unwrap();
        let harness = dir.path().join("verify.ps1");
        let artifact = dir.path().join("download.bin");
        let dest = dir.path().join("grok.exe");
        std::fs::write(&artifact, DOWNLOAD_PAYLOAD).unwrap();
        std::fs::write(&dest, PREVIOUS_GOOD).unwrap();
        let mut ps = String::new();
        ps.push_str("$ErrorActionPreference = 'Stop'\n");
        ps.push_str(
            r#"
function Download-File([string]$Url, [string]$OutFile) {
  if ($env:FAKE_MODE -eq 'missing') { throw 'checksum 404' }
  [System.IO.File]::WriteAllText($OutFile, $env:FAKE_CHECKSUM_BODY)
}
"#,
        );
        ps.push_str(get_hash);
        ps.push('\n');
        ps.push_str(parse_fn);
        ps.push('\n');
        ps.push_str(verify_fn);
        ps.push('\n');
        ps.push_str(&format!(
            "$artifact = '{}'\n$dest = '{}'\n$copied = '0'\n",
            artifact.display().to_string().replace('\'', "''"),
            dest.display().to_string().replace('\'', "''"),
        ));
        ps.push_str(
            r#"
function Copy-Item { param($Path, $Destination, $Force) $script:copied = '1' }
try {
  Verify-DownloadedSha256 $artifact 'http://127.0.0.1/artifact.sha256'
  Copy-Item -Path $artifact -Destination $dest -Force
  exit 2
} catch {
  if ($script:copied -eq '1') { Write-Error 'Copy-Item ran after a pin refuse'; exit 1 }
  $got = [System.IO.File]::ReadAllText($dest)
  $want = [System.IO.File]::ReadAllText((Join-Path (Split-Path $dest) 'want.txt'))
  if ($got -ne $want) { Write-Error 'previous-good dest bytes changed'; exit 1 }
  exit 0
}
"#,
        );
        std::fs::write(dir.path().join("want.txt"), PREVIOUS_GOOD).unwrap();
        std::fs::write(&harness, ps).unwrap();
        let fake_mode = if mode == "missing" { "missing" } else { "body" };
        let status = Command::new(pwsh)
            .arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-File")
            .arg(&harness)
            .env("FAKE_MODE", fake_mode)
            .env("FAKE_CHECKSUM_BODY", checksum_body)
            .status()
            .unwrap_or_else(|e| panic!("spawn {pwsh}: {e}"));
        assert!(
            status.success(),
            "{script}: Verify-DownloadedSha256 mode={mode} must throw, keep previous-good, and not Copy-Item"
        );
        let dest_bytes = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(dest_bytes, PREVIOUS_GOOD);
        assert_ne!(dest_bytes, DOWNLOAD_PAYLOAD);
    }
}
