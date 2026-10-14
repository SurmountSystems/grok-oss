//! grok-oss grep is embedded in `xai-grok-tools` (`grep` crate + `ignore`).
//! This crate does not cargo-install ripgrep and does not bundle a sidecar `rg`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const RG_VER: &str = "15.0.0";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GROK_SHELL_BUNDLE_RG_PATH");

    // Product search stays embedded. A local path may still be copied.
    let Some(path) = env::var("GROK_SHELL_BUNDLE_RG_PATH").ok() else {
        return Ok(());
    };

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let gen_dir = if is_bazel_build(&manifest_dir) {
        PathBuf::from(env::var("OUT_DIR")?)
    } else if let Ok(xai_root) = env::var("XAI_ROOT") {
        PathBuf::from(xai_root).join("target/tmp/grok-shell-bundle-rg")
    } else {
        PathBuf::from(env::var("OUT_DIR")?)
    };
    fs::create_dir_all(&gen_dir)?;

    println!("cargo:rustc-check-cfg=cfg(bundle_rg)");
    println!("cargo:rustc-cfg=bundle_rg");
    println!("cargo:rustc-env=GROK_SHELL_RG_VER={}", RG_VER);
    println!(
        "cargo:rustc-env=GROK_SHELL_RG_GEN_DIR={}",
        gen_dir.display()
    );
    let dest = gen_dir.join(format!("rg-{}-override.bin", RG_VER));
    println!("cargo:rustc-env=GROK_SHELL_RG_TARGET=override");
    let _ = fs::remove_file(&dest);
    fs::copy(PathBuf::from(path.clone()), &dest).map_err(|e| {
        format!(
            "Failed copying GROK_SHELL_BUNDLE_RG_PATH: {e} from path {path} to dest {}",
            dest.display()
        )
    })?;
    Ok(())
}

fn is_bazel_build(manifest_dir: &Path) -> bool {
    let manifest_dir_str = manifest_dir.to_string_lossy();
    env::var_os("BAZEL_WORKSPACE").is_some()
        || env::var_os("BUILD_WORKSPACE_DIRECTORY").is_some()
        || env::var_os("BAZEL_EXECUTION_ROOT").is_some()
        || env::var_os("BAZEL_OUTPUT_BASE").is_some()
        || manifest_dir_str.contains("/execroot/")
        || manifest_dir_str.contains("/bazel-out/")
}
