//! Build script for bundling fd (optional) and bfs/ugrep for xai-grok-tools.
//!
//! grok-oss grep is embedded Rust (`grep` crate + `ignore`), not a sidecar
//! `rg`. This script does not cargo-install ripgrep and does not copy a
//! bundled `rg` binary. Nix already supplies `pkgs.ripgrep`.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BFS_VER: &str = "4.1";
const UGREP_VER: &str = "7.7.0";
const FD_VER: &str = "10.4.2";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    bundle_rg()?;
    bundle_fd()?;
    bundle_search_tool("bfs", "BFS", BFS_VER)?;
    bundle_search_tool("ugrep", "UGREP", UGREP_VER)?;
    Ok(())
}

/// Declare `bundle_rg` and never set it.
///
/// grok-oss search is the embedded `grep` crate. This function does not
/// cargo-install ripgrep, does not download a GitHub release tarball, and
/// does not copy a bundled `rg` binary.
fn bundle_rg() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rustc-check-cfg=cfg(bundle_rg)");
    Ok(())
}

/// Embed fd when the `pi` feature is on.
///
/// A `GROK_TOOLS_BUNDLE_FD_PATH` copy uses `FD_VER` in the file name. A
/// release build without that path runs `cargo install fd-find --bin fd`.
/// A debug build without that path does not bundle. GitHub release tarballs
/// are not the install path, and this does not select a musl asset.
fn bundle_fd() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=GROK_TOOLS_BUNDLE_FD_PATH");
    println!("cargo:rustc-check-cfg=cfg(bundle_fd)");

    if env::var_os("CARGO_FEATURE_PI").is_none() {
        return Ok(());
    }

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

    let gen_dir = PathBuf::from(env::var("OUT_DIR")?).join("bundle-fd");
    fs::create_dir_all(&gen_dir)?;

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
        compress_and_pin(&dest, "FD")?;
        return Ok(());
    }

    println!("cargo:rustc-env=GROK_TOOLS_FD_TARGET=cargo-built");
    let dest = gen_dir.join(format!("fd-{FD_VER}-cargo-built.bin"));
    let _ = fs::remove_file(&dest);
    cargo_install_fd_find(&dest)?;
    compress_and_pin(&dest, "FD")?;
    Ok(())
}

/// Cargo-build `fd` from the `fd-find` crate.
///
/// Does not download a GitHub release tarball. Does not rewrite the target
/// to musl. Passes `--target` only when Cargo already set `TARGET`.
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

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn compress_and_pin(
    dest: &std::path::Path,
    name_uc: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(dest)?;
    let sha = {
        use sha2::Digest as _;
        hex_encode(&sha2::Sha256::digest(&bytes))
    };

    let compressed = zstd::encode_all(bytes.as_slice(), 19)?;
    let mut zst = dest.to_path_buf().into_os_string();
    zst.push(".zst");
    fs::write(&zst, &compressed)?;

    println!("cargo:rustc-env=GROK_TOOLS_{name_uc}_SHA256={sha}");
    Ok(())
}

/// Bundle a prebuilt static search-tool binary (`bfs`/`ugrep`) when
/// `GROK_TOOLS_BUNDLE_<NAME>_PATH` points at one (supplied by the release
/// pipeline). Emits `cfg(bundle_<name>)` so the crate's `include_bytes!` +
/// self-extract engages. No auto-download: bfs/ugrep publish no prebuilt
/// static release assets, so the release pipeline supplies the path.
fn bundle_search_tool(
    name: &str,
    name_uc: &str,
    ver: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let override_env = format!("GROK_TOOLS_BUNDLE_{name_uc}_PATH");
    println!("cargo:rerun-if-env-changed={override_env}");
    println!("cargo:rustc-check-cfg=cfg(bundle_{name})");

    // The consumer (`embedded_search_tools`) is `#[cfg(unix)]`, so embedding on a
    // Windows target is dead weight.
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
    compress_and_pin(&dest, name_uc)?;
    Ok(())
}
