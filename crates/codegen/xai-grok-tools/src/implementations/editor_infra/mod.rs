//! Shared editor infrastructure used by the default grok_build tools.
pub mod file_operation_lock;
pub mod per_path_write_lock;
pub use file_operation_lock::{FileOperationLockGuard, FileOperationLockManager};
pub use per_path_write_lock::{
    PathHeldError, PerPathWriteGuard, acquire_for_tool, acquire_paths_for_tool, try_acquire_write,
    try_acquire_writes,
};
#[cfg(test)]
mod per_path_write_lock_tests;
/// Default `block_until_ms` when the model omits the field on blocking shell
/// tools. Shared so ACP UI labels and tool implementations stay aligned.
pub const DEFAULT_BLOCK_UNTIL_MS: u64 = 30_000;
