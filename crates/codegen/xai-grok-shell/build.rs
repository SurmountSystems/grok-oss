//! Build script for bundling ripgrep for the grok-shell crate.
//!
//! - If `GROK_SHELL_BUNDLE_RG_PATH` is set, copy that cargo-built or Nix `rg`
//! - Otherwise, only bundle in release builds by cargo-installing the
//!   `ripgrep` crate (not a GitHub musl tarball)
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RG_VER: &str = "15.0.0";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Only bundle in release builds to avoid slowing down cargo check.
    println!("cargo:rerun-if-env-changed=GROK_SHELL_BUNDLE_RG_PATH");
    println!("cargo:rustc-check-cfg=cfg(bundle_rg)");

    // Decide whether to bundle: path override OR release build. Bail before
    // touching the filesystem so debug `cargo check` needs no environment.
    let path_override = env::var("GROK_SHELL_BUNDLE_RG_PATH")
        .ok()
        .filter(|s| !s.is_empty());
    let is_release = env::var("PROFILE").as_deref() == Ok("release");
    if path_override.is_none() && !is_release {
        return Ok(());
    }

    // In Bazel builds, write into OUT_DIR (which is writable) rather than
    // XAI_ROOT/target/tmp (which is read-only inside the sandbox). Outside
    // Bazel, prefer XAI_ROOT's shared cache dir (monorepo behavior) and fall
    // back to OUT_DIR for standalone checkouts where XAI_ROOT is not a thing.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let in_bazel = is_bazel_build(&manifest_dir);
    let gen_dir = if in_bazel {
        PathBuf::from(env::var("OUT_DIR")?)
    } else if let Ok(xai_root) = env::var("XAI_ROOT") {
        PathBuf::from(xai_root).join("target/tmp/grok-shell-bundle-rg")
    } else {
        PathBuf::from(env::var("OUT_DIR")?)
    };
    fs::create_dir_all(&gen_dir)?;

    // Skip auto-bundling on Windows: runtime falls back to `rg` on PATH.
    // An explicit GROK_SHELL_BUNDLE_RG_PATH still copies any binary.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" && path_override.is_none() {
        return Ok(());
    }

    println!("cargo:rustc-cfg=bundle_rg");
    println!("cargo:rustc-env=GROK_SHELL_RG_VER={RG_VER}");
    println!(
        "cargo:rustc-env=GROK_SHELL_RG_GEN_DIR={}",
        gen_dir.display()
    );

    if let Some(path) = path_override {
        let dest = gen_dir.join(format!("rg-{RG_VER}-override.bin"));
        println!("cargo:rustc-env=GROK_SHELL_RG_TARGET=override");
        let _ = fs::remove_file(&dest);
        fs::copy(PathBuf::from(path.clone()), &dest).map_err(|e| {
            format!(
                "Failed copying GROK_SHELL_BUNDLE_RG_PATH: {e} from path {path} to dest {}",
                dest.display()
            )
        })?;
        return Ok(());
    }

    println!("cargo:rustc-env=GROK_SHELL_RG_TARGET=cargo-built");
    let dest = gen_dir.join(format!("rg-{RG_VER}-cargo-built.bin"));
    let _ = fs::remove_file(&dest);
    cargo_install_ripgrep(&dest)?;
    Ok(())
}

/// Cargo-build `rg` from the `ripgrep` crate via the delayed crate index.
/// Does not download a GitHub release tarball. Does not rewrite GNU to musl.
fn cargo_install_ripgrep(dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let cargo = env::var("CARGO").map_err(|_| "CARGO is unset")?;
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let root = out_dir.join("cargo-install-rg");
    let target_dir = out_dir.join("cargo-install-rg-target");
    fs::create_dir_all(&root)?;
    fs::create_dir_all(&target_dir)?;

    let mut cmd = Command::new(&cargo);
    cmd.arg("install")
        .arg("--version")
        .arg(RG_VER)
        .arg("--locked")
        .arg("--no-track")
        .arg("--force")
        .arg("--root")
        .arg(&root)
        .arg("--bin")
        .arg("rg")
        .arg("ripgrep")
        .env("CARGO_TARGET_DIR", &target_dir);
    cmd.env_remove("RUSTFLAGS");
    cmd.env_remove("CARGO_ENCODED_RUSTFLAGS");
    cmd.env_remove("GROK_TOOLS_BUNDLE_RG_PATH");
    cmd.env_remove("GROK_SHELL_BUNDLE_RG_PATH");
    if let Ok(target) = env::var("TARGET") {
        if !target.is_empty() {
            cmd.arg("--target").arg(target);
        }
    }

    let status = cmd
        .status()
        .map_err(|e| format!("failed to spawn cargo install ripgrep {RG_VER}: {e}"))?;
    if !status.success() {
        return Err(format!(
            "cargo install ripgrep {RG_VER} failed with {status}. Bundled rg is cargo-built from the ripgrep crate; GitHub musl tarball is not the install path."
        )
        .into());
    }

    let bin = if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        root.join("bin").join("rg.exe")
    } else {
        root.join("bin").join("rg")
    };
    fs::copy(&bin, dest).map_err(|e| {
        format!(
            "copy cargo-built rg from {} to {}: {e}",
            bin.display(),
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
