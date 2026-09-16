//! Build script for bundling ripgrep and fd for the xai-grok-tools crate.
//!
//! - If `GROK_TOOLS_BUNDLE_RG_PATH` is set, copy that cargo-built or Nix `rg`
//! - Otherwise, only bundle in release builds by cargo-installing the
//!   `ripgrep` crate (not a GitHub musl tarball)
//! - fd: same crate-build path from `fd-find` when the `pi` feature is on
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RG_VER: &str = "15.0.0";
const BFS_VER: &str = "4.1";
const UGREP_VER: &str = "7.7.0";
const FD_VER: &str = "10.4.2";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    bundle_rg()?;
    // fd is an optional vendored file-search binary backing a feature-gated
    // toolset; skip the crate-build/embed entirely when that feature is off
    // (shipped TUI binaries).
    if env::var_os("CARGO_FEATURE_PI").is_some() {
        bundle_fd()?;
    }
    // bfs/ugrep back the bash-harness find/grep shadows (embedded_search_tools).
    bundle_search_tool("bfs", "BFS", BFS_VER)?;
    bundle_search_tool("ugrep", "UGREP", UGREP_VER)?;
    Ok(())
}

/// Embed fd. Path override copies a cargo-built or Nix `fd`. Release without
/// a path cargo-installs the `fd-find` crate (host GNU unless cargo `TARGET`
/// is already something else). GitHub musl tarball is not the install path.
fn bundle_fd() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=GROK_TOOLS_BUNDLE_FD_PATH");
    println!("cargo:rustc-check-cfg=cfg(bundle_fd)");

    let gen_dir = PathBuf::from(env::var("OUT_DIR")?).join("bundle-fd");
    fs::create_dir_all(&gen_dir)?;

    // The consuming vendor extraction is unix-only. Never bundle on Windows.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        return Ok(());
    }

    let path_override = env::var("GROK_TOOLS_BUNDLE_FD_PATH")
        .ok()
        .filter(|s| !s.is_empty());
    let is_release = env::var("PROFILE").as_deref() == Ok("release");
    if path_override.is_none() && !is_release {
        return Ok(());
    }

    println!("cargo:rustc-cfg=bundle_fd");
    println!("cargo:rustc-env=GROK_TOOLS_FD_VER={FD_VER}");

    if let Some(path) = path_override {
        let dest = gen_dir.join(format!("fd-{FD_VER}-override.bin"));
        println!("cargo:rustc-env=GROK_TOOLS_FD_TARGET=override");
        let _ = fs::remove_file(&dest);
        fs::copy(PathBuf::from(path.clone()), &dest).map_err(|e| {
            format!(
                "Failed copying GROK_TOOLS_BUNDLE_FD_PATH: {e} from path {path} to dest {}",
                dest.display()
            )
        })?;
        return Ok(());
    }

    println!("cargo:rustc-env=GROK_TOOLS_FD_TARGET=cargo-built");
    let dest = gen_dir.join(format!("fd-{FD_VER}-cargo-built.bin"));
    let _ = fs::remove_file(&dest);
    cargo_install_fd_find(&dest)?;
    Ok(())
}

/// Cargo-build `fd` from the `fd-find` crate via the delayed crate index.
/// Does not download a GitHub release tarball. Does not rewrite GNU to musl.
fn cargo_install_fd_find(dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let cargo = env::var("CARGO").map_err(|_| "CARGO is unset")?;
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let root = out_dir.join("cargo-install-fd");
    let target_dir = out_dir.join("cargo-install-fd-target");
    fs::create_dir_all(&root)?;
    fs::create_dir_all(&target_dir)?;

    let mut cmd = Command::new(&cargo);
    cmd.arg("install")
        .arg("--version")
        .arg(FD_VER)
        .arg("--locked")
        .arg("--no-track")
        .arg("--force")
        .arg("--root")
        .arg(&root)
        .arg("--bin")
        .arg("fd")
        .arg("fd-find")
        .env("CARGO_TARGET_DIR", &target_dir);
    cmd.env_remove("RUSTFLAGS");
    cmd.env_remove("CARGO_ENCODED_RUSTFLAGS");
    cmd.env_remove("GROK_TOOLS_BUNDLE_FD_PATH");
    if let Ok(target) = env::var("TARGET") {
        if !target.is_empty() {
            cmd.arg("--target").arg(target);
        }
    }

    let status = cmd
        .status()
        .map_err(|e| format!("failed to spawn cargo install fd-find {FD_VER}: {e}"))?;
    if !status.success() {
        return Err(format!(
            "cargo install fd-find {FD_VER} failed with {status}. Bundled fd is cargo-built from the fd-find crate; GitHub musl tarball is not the install path."
        )
        .into());
    }

    let bin = root.join("bin").join("fd");
    fs::copy(&bin, dest).map_err(|e| {
        format!(
            "copy cargo-built fd from {} to {}: {e}",
            bin.display(),
            dest.display()
        )
    })?;
    Ok(())
}

/// Bundle a prebuilt **static** search-tool binary (`bfs`/`ugrep`) when
/// `GROK_TOOLS_BUNDLE_<NAME>_PATH` points at one (supplied by the release
/// pipeline). Emits
/// `cfg(bundle_<name>)` so the crate's `include_bytes!` + self-extract engages.
///
/// No auto-download: bfs/ugrep publish no prebuilt static
/// release assets, so the release pipeline supplies the path. Unset → not
/// bundled (the runtime resolver falls back to `~/.grok/vendor` / `$PATH`);
/// never a hard failure, so an un-wired build still succeeds.
fn bundle_search_tool(
    name: &str,
    name_uc: &str,
    ver: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let override_env = format!("GROK_TOOLS_BUNDLE_{name_uc}_PATH");
    println!("cargo:rerun-if-env-changed={override_env}");
    // Always declare the cfg so `#[cfg(bundle_<name>)]` is lint-clean when unset.
    println!("cargo:rustc-check-cfg=cfg(bundle_{name})");

    // The consumer (`embedded_search_tools`) is `#[cfg(unix)]`, so embedding on a
    // Windows target is dead weight — skip (mirrors the ripgrep Windows skip).
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        return Ok(());
    }

    let Some(src) = env::var(&override_env).ok().filter(|s| !s.is_empty()) else {
        return Ok(());
    };

    let gen_dir = PathBuf::from(env::var("OUT_DIR")?).join(format!("bundle-{name}"));
    fs::create_dir_all(&gen_dir)?;
    let dest = gen_dir.join(format!("{name}-{ver}-override.bin"));
    let _ = fs::remove_file(&dest);
    fs::copy(&src, &dest)
        .map_err(|e| format!("copy {override_env} from {src} to {}: {e}", dest.display()))?;

    println!("cargo:rustc-cfg=bundle_{name}");
    println!("cargo:rustc-env=GROK_TOOLS_{name_uc}_VER={ver}");
    println!("cargo:rustc-env=GROK_TOOLS_{name_uc}_TARGET=override");
    Ok(())
}

/// Embed ripgrep. Path override copies a cargo-built or Nix `rg`. Release
/// without a path cargo-installs the `ripgrep` crate (host GNU unless cargo
/// `TARGET` is already something else). GitHub musl tarball is not the
/// install path.
fn bundle_rg() -> Result<(), Box<dyn std::error::Error>> {
    // Only bundle in release builds to avoid slowing down cargo check.
    println!("cargo:rerun-if-env-changed=GROK_TOOLS_BUNDLE_RG_PATH");
    println!("cargo:rustc-check-cfg=cfg(bundle_rg)");

    let gen_dir = PathBuf::from(env::var("OUT_DIR")?).join("bundle-rg");
    fs::create_dir_all(&gen_dir)?;

    let path_override = env::var("GROK_TOOLS_BUNDLE_RG_PATH")
        .ok()
        .filter(|s| !s.is_empty());
    let is_release = env::var("PROFILE").as_deref() == Ok("release");
    if path_override.is_none() && !is_release {
        return Ok(());
    }

    // Skip auto-bundling on Windows: runtime falls back to `rg` on PATH.
    // An explicit GROK_TOOLS_BUNDLE_RG_PATH still copies any binary.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" && path_override.is_none() {
        return Ok(());
    }

    println!("cargo:rustc-cfg=bundle_rg");
    println!("cargo:rustc-env=GROK_TOOLS_RG_VER={RG_VER}");

    if let Some(path) = path_override {
        let dest = gen_dir.join(format!("rg-{RG_VER}-override.bin"));
        println!("cargo:rustc-env=GROK_TOOLS_RG_TARGET=override");
        let _ = fs::remove_file(&dest);
        fs::copy(PathBuf::from(path.clone()), &dest).map_err(|e| {
            format!(
                "Failed copying GROK_TOOLS_BUNDLE_RG_PATH: {e} from path {path} to dest {}",
                dest.display()
            )
        })?;
        return Ok(());
    }

    println!("cargo:rustc-env=GROK_TOOLS_RG_TARGET=cargo-built");
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
