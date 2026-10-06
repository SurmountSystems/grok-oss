// Resolution (bundled binary, RG_BIN_PATH, Bazel runfiles, PATH) lives in the grok-tools crate.
// This module only preserves the `crate::util::ripgrep` path.
// `rg_path` is the public function on the grep ripgrep module, not `util`.
pub use xai_grok_tools::implementations::grok_build::grep::ripgrep::rg_path;
