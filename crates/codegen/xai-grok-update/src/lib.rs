#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    unreachable_code,
    dead_code
)]
#![deny(clippy::indexing_slicing)]
pub mod auto_update;
mod cleanup_downloads;
pub mod oss_update;
pub mod version;
mod version_policy;
mod winget;
pub use auto_update::UpdateStatus;
pub use oss_update::{
    OSS_GITHUB_REPO, OssUpdateStatus, check_against_main, format_build_id, how_to_update_message,
    print_oss_update_status,
};
pub use version::{UpdateConfig, channel_label, channel_name, write_version_cache};
pub use version_policy::enforce_version_policy_or_exit;
