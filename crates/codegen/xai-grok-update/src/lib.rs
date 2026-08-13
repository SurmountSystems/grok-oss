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
    InstallBackend, InstallStdioPolicy, REBUILD_PROGRESS_LINE_MAX_CHARS, RebuildProgressEngine,
    RebuildProgressEvent, RebuildReport, cargo_sub_fraction, clamp_rebuild_fraction,
    format_rebuild_cli_progress, install_stdio_policy, is_cargo_json_build_finished,
    is_rebuild_progress_stage_line, is_stable_height_progress_line, overall_fraction_in_cargo,
    parse_cargo_json_artifact_package, parse_compiling_crate, rebuild_and_relaunch,
    rebuild_and_relaunch_with_progress, rebuild_progress_bar_chars, rebuild_progress_weights,
    resolve_source_root, run_install, run_install_with_progress, sanitize_rebuild_progress_line,
    verify_installed_identity,
};
pub use version::{UpdateConfig, channel_label, channel_name, write_version_cache};
pub use version_policy::enforce_version_policy_or_exit;
