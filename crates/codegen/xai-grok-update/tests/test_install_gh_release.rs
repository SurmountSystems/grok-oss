//! End-to-end tests for `install_gh_release`.
//!
//! Pins SHA-256 of the published GitHub release asset `${artifact}.sha256`
//! (fail-closed on miss, unreadable digest, or mismatch). Not SHA-1.
//! Previous-good stays until verify succeeds. Reuses
//! `xai_grok_update::artifact_sha256` (no second acceptance set).

#![cfg(unix)]

mod common;

use serial_test::serial;

use common::{
    FakeBinGuard, can_exec_shell_scripts, host_platform, previous_good_artifact, reset_home,
    small_good_artifact, small_good_artifact_sha256, small_good_artifact_sha256_line, test_home,
};
use xai_grok_telemetry::events::CliUpdateErrorKind;
use xai_grok_update::artifact_sha256::{
    artifact_checksum_url, parse_sha256_file_bytes, sha256_hex,
};
use xai_grok_update::auto_update::{classify_install_error, install_gh_release};

fn setup_gh() -> FakeBinGuard {
    let _ = test_home();
    reset_home();
    FakeBinGuard::install_gh_serving_releases()
}

fn seed_previous_good(platform: &str) {
    let home = test_home();
    let downloads = home.join("downloads");
    let bin = home.join("bin");
    std::fs::create_dir_all(&downloads).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    let prev = downloads.join(format!("grok-0.1.100-{platform}"));
    std::fs::write(&prev, previous_good_artifact()).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&prev, std::fs::Permissions::from_mode(0o755)).unwrap();
    let rel = format!("../downloads/grok-0.1.100-{platform}");
    std::os::unix::fs::symlink(&rel, bin.join("grok")).unwrap();
    std::os::unix::fs::symlink(&rel, bin.join("agent")).unwrap();
}

fn assert_previous_good_stays(platform: &str) {
    let home = test_home();
    let grok = std::fs::read_link(home.join("bin").join("grok")).unwrap();
    assert!(
        grok.to_string_lossy().contains("0.1.100"),
        "SHA-256 refuse must keep previous-good, got {grok:?}"
    );
    let path = home
        .join("downloads")
        .join(format!("grok-0.1.100-{platform}"));
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        bytes,
        previous_good_artifact(),
        "refused install must keep previous-good bytes, not the download"
    );
    assert_ne!(
        bytes,
        small_good_artifact(),
        "previous-good must stay distinct from the download payload"
    );
}

/// Named contract: a published SHA-256 that does not match the downloaded
/// bytes must refuse the install. Previous-good stays. Not SHA-1.
#[tokio::test]
#[serial]
async fn install_gh_release_refuses_when_sha256_does_not_match() {
    if !can_exec_shell_scripts() {
        eprintln!("skipping: shell scripts cannot execute in this sandbox");
        return;
    }
    let g = setup_gh();
    let platform = host_platform();
    seed_previous_good(&platform);
    g.set_gh_artifact(&small_good_artifact());
    g.set_gh_sha256_body(
        "0000000000000000000000000000000000000000000000000000000000000000  grok\n",
    );

    let err = install_gh_release(Some("0.1.181"))
        .await
        .expect_err("SHA-256 mismatch must refuse the install");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("SHA-256 mismatch"),
        "expected mismatch refuse, got: {msg}"
    );
    assert_eq!(classify_install_error(&err), CliUpdateErrorKind::Download);
    assert_previous_good_stays(&platform);
    let dest = test_home()
        .join("downloads")
        .join(format!("grok-0.1.181-{platform}"));
    assert!(
        !dest.exists(),
        "mismatch must not publish the unverified blob onto previous dest: {dest:?}"
    );
}

/// Named contract: no published SHA-256 GitHub release asset means
/// fail-closed. Do not install an unverified blob. Not SHA-1.
#[tokio::test]
#[serial]
async fn install_gh_release_refuses_when_sha256_checksum_file_is_missing() {
    if !can_exec_shell_scripts() {
        eprintln!("skipping: shell scripts cannot execute in this sandbox");
        return;
    }
    let g = setup_gh();
    let platform = host_platform();
    seed_previous_good(&platform);
    g.set_gh_artifact(&small_good_artifact());
    g.set_gh_sha256_missing();

    let err = install_gh_release(Some("0.1.181"))
        .await
        .expect_err("missing SHA-256 file must refuse the install");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("checksum file missing") || msg.contains("SHA-256 checksum file missing"),
        "expected missing checksum refuse, got: {msg}"
    );
    assert_eq!(classify_install_error(&err), CliUpdateErrorKind::Download);
    assert_previous_good_stays(&platform);
}

/// Named contract: an unreadable published checksum (empty, not 64 hex,
/// tagged openssl form, 63-char hex, 40-hex SHA-1) is fail-closed.
#[tokio::test]
#[serial]
async fn install_gh_release_refuses_when_sha256_checksum_file_is_unreadable() {
    if !can_exec_shell_scripts() {
        eprintln!("skipping: shell scripts cannot execute in this sandbox");
        return;
    }
    let bodies = [
        "",
        "not-a-digest\n",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde\n",
        "SHA256 (grok) = 306c6ca7407560340797866e077e053627ad409277d1b9da58106fce4cf717cb\n",
        "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg\n",
        "0123456789abcdef0123456789abcdef01234567\n",
    ];
    let platform = host_platform();
    for body in bodies {
        let g = setup_gh();
        seed_previous_good(&platform);
        g.set_gh_artifact(&small_good_artifact());
        g.set_gh_sha256_body(body);
        assert_eq!(
            parse_sha256_file_bytes(body.as_bytes()),
            None,
            "fixture must be unreadable under the shared SHA-256 parser: {body:?}"
        );

        let err = install_gh_release(Some("0.1.181"))
            .await
            .expect_err("unreadable SHA-256 file must refuse the install");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("not a SHA-256 hex digest"),
            "expected unreadable checksum refuse for {body:?}, got: {msg}"
        );
        assert_eq!(classify_install_error(&err), CliUpdateErrorKind::Download);
        assert_previous_good_stays(&platform);
    }
}

/// Happy path plus asset-name contract: matching SHA-256 installs, and the
/// installer fetches the published `.sha256` GitHub release asset.
#[tokio::test]
#[serial]
async fn install_gh_release_installs_when_published_sha256_matches() {
    if !can_exec_shell_scripts() {
        eprintln!("skipping: shell scripts cannot execute in this sandbox");
        return;
    }
    let g = setup_gh();
    let platform = host_platform();
    seed_previous_good(&platform);
    g.set_gh_artifact(&small_good_artifact());
    g.set_gh_sha256_body(&small_good_artifact_sha256_line());
    assert_eq!(
        parse_sha256_file_bytes(small_good_artifact_sha256_line().as_bytes()).as_deref(),
        Some(small_good_artifact_sha256().as_str())
    );
    assert_ne!(previous_good_artifact(), small_good_artifact());

    install_gh_release(Some("0.1.181")).await.unwrap();

    let home = test_home();
    let downloaded = home
        .join("downloads")
        .join(format!("grok-0.1.181-{platform}"));
    assert_eq!(std::fs::read(&downloaded).unwrap(), small_good_artifact());
    let target = std::fs::read_link(home.join("bin").join("grok")).unwrap();
    assert!(
        target.to_string_lossy().contains("0.1.181"),
        "matching SHA-256 must activate the new binary: {target:?}"
    );
    assert_eq!(
        small_good_artifact_sha256(),
        sha256_hex(&small_good_artifact())
    );

    let args = g.args_log();
    let checksum_pattern = artifact_checksum_url(&format!("grok-0.1.181-{platform}"));
    assert!(
        args.iter()
            .any(|l| l.contains("release download") && l.contains(&checksum_pattern)),
        "must fetch {checksum_pattern} via gh, args:\n{args:?}"
    );
    assert!(
        args.iter().all(|l| !l.contains(".sha1")),
        "must not fetch SHA-1 checksums, args:\n{args:?}"
    );
    assert!(
        args.iter()
            .any(|l| l.contains("release download") && !l.contains(".sha256")),
        "must still download the binary asset, args:\n{args:?}"
    );
}
