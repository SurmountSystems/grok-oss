//! Grok OSS update, rebuild, and optional SpaceXAI auto-update.
//!
//! Hash roles, in complete thoughts:
//! - Git object ids (40-hex SHA-1 on today's repos, or a future git SHA-256
//!   object id) identify commits. They are not a security hash for downloads
//!   or Nix FODs.
//! - `/rebuild` checks the installed binary with `--version`, then compares
//!   package version plus that git identity.
//! - The SpaceXAI internal auto-updater and the GitHub Releases installer
//!   pin SHA-256 of the published `${artifact}.sha256` file (fail-closed on
//!   miss or mismatch). Internal then still smoke-tests `--version`. They
//!   do not hash the bytes with SHA-1. GitHub publishes that pin as a
//!   release asset named `${artifact}.sha256`.
//! - New artifact / FOD verify is SHA-256 or minisign. POSIX `install.sh`
//!   / `install-enterprise.sh` and PowerShell `install.ps1` /
//!   `install-enterprise.ps1` pin SHA-256 of the published checksum file.

pub mod artifact_sha256;
pub mod auto_update;
pub mod oss_update;
pub mod rebuild;
pub mod version;
mod version_policy;

pub use auto_update::UpdateStatus;
pub use oss_update::{
    OSS_GITHUB_REPO, OssUpdateStatus, check_against_main, format_build_id, how_to_update_message,
    print_oss_update_status,
};
pub use rebuild::{
    InstallBackend, InstallStdioPolicy, PeerRelaunchOutcome, REBUILD_PROGRESS_LINE_MAX_CHARS,
    RebuildFleetPlan, RebuildFleetSignalStep, RebuildProgressEngine, RebuildProgressEvent,
    RebuildRelaunchRequest, RebuildReport, backup_previous_grok_oss_binary, cargo_sub_fraction,
    clamp_rebuild_fraction, collect_rebuild_signal_pids, export_git_index_to,
    format_rebuild_cli_progress, git_work_tree_is_present, install_stdio_policy,
    is_cargo_json_build_finished, is_rebuild_progress_stage_line, is_stable_height_progress_line,
    make_rebuild_relaunch_request, overall_fraction_in_cargo, parse_cargo_json_artifact_package,
    parse_compiling_crate, peer_pids_to_signal_for_relaunch, peer_rebuild_request_is_actionable,
    pid_is_grok_oss_product, previous_grok_oss_backup_path, read_rebuild_relaunch_request,
    read_rebuild_relaunch_request_in, rebuild_and_relaunch, rebuild_and_relaunch_with_progress,
    rebuild_fleet_signal_steps, rebuild_progress_bar_chars, rebuild_progress_weights,
    rebuild_relaunch_request_path, resolve_source_root, run_install, run_install_with_progress,
    running_exe_needs_relaunch_onto, sanitize_rebuild_progress_line,
    should_peer_relaunch_for_request, should_peer_relaunch_for_request_with_current_exe,
    signal_active_sessions_to_relaunch, stash_pop_rebuild_unstaged, stash_unstaged_keep_index,
    verify_installed_identity, write_rebuild_relaunch_request, write_rebuild_relaunch_request_in,
};
pub use version::{UpdateConfig, channel_label, channel_name, write_version_cache};
pub use version_policy::enforce_version_policy_or_exit;

#[cfg(test)]
mod security_hash_tests {
    /// Contract: this crate must not take the `sha1` hasher for download
    /// verify. Git object ids stay as strings from git / GitHub. Artifact
    /// installers, the internal auto-updater, and the GitHub Releases
    /// installer pin SHA-256 of a published checksum file, not SHA-1.
    #[test]
    fn crate_manifest_does_not_depend_on_sha1_hasher() {
        let toml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        for line in toml.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            assert!(
                !trimmed.contains("sha1"),
                "xai-grok-update must not depend on sha1 for artifact verify: {trimmed}"
            );
        }
    }
}
