//! Build script for bundling fd (optional) and bfs/ugrep for xai-grok-tools.
//!
//! grok-oss grep is embedded Rust (`grep` crate + `ignore`), not a sidecar
//! `rg`. This script does not cargo-install ripgrep and does not copy a
//! bundled `rg` binary. Nix already supplies `pkgs.ripgrep`.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "src/util/pdf_docx_skill_roots.rs"]
mod pdf_docx_skill_roots;

const BFS_VER: &str = "4.1";
const UGREP_VER: &str = "7.7.0";
const FD_VER: &str = "10.4.2";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/bin/lsp_mock_server.rs");
    println!("cargo:rerun-if-env-changed=GROK_BUILDING_LSP_MOCK_SERVER");
    // Nested `cargo build --bin lsp_mock_server` sets the guard and uses a
    // target dir named `lsp-mock-target`. Only that OUT_DIR may return
    // before compile. The outer OUT_DIR is the path LSP tests launch
    // (`.../out/lsp_mock_server`). A leaked guard must not skip that copy.
    if lsp_mock_nested_reentry() {
        return Ok(());
    }
    pdf_docx_skill_roots::ensure_pdf_docx_skill_roots()?;
    bundle_rg()?;
    bundle_fd()?;
    bundle_search_tool("bfs", "BFS", BFS_VER)?;
    bundle_search_tool("ugrep", "UGREP", UGREP_VER)?;
    compile_lsp_mock_server()?;
    Ok(())
}

/// True only for the nested mock rebuild.
///
/// `GROK_BUILDING_LSP_MOCK_SERVER` on the outer invocation must not skip
/// the compile. The outer OUT_DIR does not contain `lsp-mock-target`.
fn lsp_mock_nested_reentry() -> bool {
    if env::var_os("GROK_BUILDING_LSP_MOCK_SERVER").is_none()
        && env::var_os("GROK_LSP_MOCK_REENTRY").is_none()
    {
        return false;
    }
    let Ok(out_dir) = env::var("OUT_DIR") else {
        return false;
    };
    Path::new(&out_dir)
        .components()
        .any(|component| component.as_os_str() == "lsp-mock-target")
}

/// Compile `src/bin/lsp_mock_server.rs` and leave the executable at
/// `OUT_DIR/lsp_mock_server` before this function returns.
///
/// That is the path the handshake opens
/// (`debug/build/xai-grok-tools/<hash>/out/lsp_mock_server`). Nested cargo
/// may write the bin under its own target dir, a host triple, or `deps/`.
/// Those paths are not the handshake path. This copies the ELF to `OUT_DIR`.
fn compile_lsp_mock_server() -> Result<(), Box<dyn std::error::Error>> {
    let cargo = env::var("CARGO").map_err(|_| "CARGO is unset")?;
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let dest = out_dir.join("lsp_mock_server");
    // Keep the nested target beside the profile dir, not inside OUT_DIR.
    // The last check spawned OUT_DIR/lsp_mock_server and the file was gone.
    // A target tree inside OUT_DIR is removed when Cargo rebuilds that dir.
    let target_dir = match profile_dir_from_out_dir(&out_dir) {
        Some(dir) => dir.join("lsp-mock-target"),
        None => out_dir.join("lsp-mock-target"),
    };
    fs::create_dir_all(&target_dir)?;

    let outer_target = env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    let release = env::var("PROFILE").as_deref() == Ok("release");
    let target = env::var("TARGET").unwrap_or_default();
    let host = env::var("HOST").unwrap_or_default();
    let cross = !target.is_empty() && !host.is_empty() && target != host;
    let profile = if release { "release" } else { "debug" };

    let mut cmd = Command::new(&cargo);
    cmd.arg("build")
        .arg("--manifest-path")
        .arg(manifest_dir.join("Cargo.toml"))
        .arg("--bin")
        .arg("lsp_mock_server")
        .arg("--locked")
        .arg("--target-dir")
        .arg(&target_dir)
        .arg("--message-format=json")
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("GROK_BUILDING_LSP_MOCK_SERVER", "1")
        .env("GROK_LSP_MOCK_REENTRY", "1")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("CLIPPY_ARGS")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if release {
        cmd.arg("--release");
    }
    if cross {
        cmd.arg("--target").arg(&target);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("failed to spawn cargo build --bin lsp_mock_server: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo build --bin lsp_mock_server failed with {}: {}",
            output.status,
            stderr_tail(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut candidates = executables_from_cargo_json(&stdout);
    push_profile_candidates(&target_dir, &target, &host, profile, cross, &mut candidates);
    collect_bins_under(&target_dir, &mut candidates);
    if let Some(outer) = &outer_target {
        push_profile_candidates(outer, &target, &host, profile, cross, &mut candidates);
        collect_bins_named(&outer.join(profile), &mut candidates);
        collect_bins_named(&outer.join(profile).join("deps"), &mut candidates);
    }

    let mut missed = Vec::new();
    let mut built = None;
    for path in candidates {
        if path == dest {
            continue;
        }
        if is_real_elf(&path) {
            built = Some(path);
            break;
        }
        if missed.len() < 20 {
            missed.push(path);
        }
    }
    let built = built.ok_or_else(|| {
        format!(
            "lsp_mock_server ELF was not produced; {} would stay missing. nested target {}, outer {}. checked {}",
            dest.display(),
            target_dir.display(),
            outer_target
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "(unset)".to_string()),
            if missed.is_empty() {
                "(no candidate paths)".to_string()
            } else {
                missed
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        )
    })?;

    copy_lsp_mock_binary(&built, &dest)?;
    if !is_real_elf(&dest) {
        return Err(format!("copy did not leave an ELF at {}", dest.display()).into());
    }
    if let Some(profile_dir) = profile_dir_from_out_dir(&out_dir) {
        let beside_tests = profile_dir.join("lsp_mock_server");
        if beside_tests != dest {
            let _ = copy_lsp_mock_binary(&built, &beside_tests);
        }
    }
    Ok(())
}

fn stderr_tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let count = text.chars().count();
    if count <= 2000 {
        return text.into_owned();
    }
    text.chars().skip(count - 2000).collect()
}

fn executables_from_cargo_json(stdout: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for line in stdout.lines() {
        if !line.contains("lsp_mock_server") {
            continue;
        }
        if let Some(path) = executable_field(line) {
            paths.push(path);
        }
    }
    paths
}

fn executable_field(line: &str) -> Option<PathBuf> {
    let key = "\"executable\":\"";
    let start = line.find(key)? + key.len();
    let mut path = String::new();
    let mut escaped = false;
    for ch in line[start..].chars() {
        if escaped {
            path.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            break;
        }
        path.push(ch);
    }
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn push_profile_candidates(
    target_dir: &Path,
    target: &str,
    host: &str,
    profile: &str,
    cross: bool,
    candidates: &mut Vec<PathBuf>,
) {
    if cross && !target.is_empty() {
        candidates.push(
            target_dir
                .join(target)
                .join(profile)
                .join("lsp_mock_server"),
        );
    }
    candidates.push(target_dir.join(profile).join("lsp_mock_server"));
    if !target.is_empty() {
        candidates.push(
            target_dir
                .join(target)
                .join(profile)
                .join("lsp_mock_server"),
        );
    }
    if !host.is_empty() {
        candidates.push(target_dir.join(host).join(profile).join("lsp_mock_server"));
    }
    collect_bins_named(&target_dir.join(profile).join("deps"), candidates);
    if !target.is_empty() {
        collect_bins_named(
            &target_dir.join(target).join(profile).join("deps"),
            candidates,
        );
    }
    if !host.is_empty() {
        collect_bins_named(
            &target_dir.join(host).join(profile).join("deps"),
            candidates,
        );
    }
}

fn collect_bins_named(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if is_mock_bin_name(&name) {
            out.push(entry.path());
        }
    }
}

fn collect_bins_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if name == "build" || name == ".fingerprint" || name == "incremental" {
                continue;
            }
            collect_bins_under(&entry.path(), out);
            continue;
        }
        if file_type.is_file() && is_mock_bin_name(&name) {
            out.push(entry.path());
        }
    }
}

fn is_mock_bin_name(name: &str) -> bool {
    name == "lsp_mock_server" || (name.starts_with("lsp_mock_server-") && !name.contains('.'))
}

fn is_real_elf(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() < 1024 {
        return false;
    }
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 4];
    std::io::Read::read_exact(&mut file, &mut magic).is_ok() && magic == [0x7f, b'E', b'L', b'F']
}

fn profile_dir_from_out_dir(out_dir: &Path) -> Option<&Path> {
    let mut cursor = out_dir;
    while let Some(parent) = cursor.parent() {
        if cursor.file_name().and_then(|name| name.to_str()) == Some("build") {
            return Some(parent);
        }
        cursor = parent;
    }
    None
}

fn copy_lsp_mock_binary(built: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !is_real_elf(built) {
        return Err(format!("not an ELF: {}", built.display()).into());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_file_name("lsp_mock_server.copying");
    let _ = fs::remove_file(&tmp);
    fs::copy(built, &tmp).map_err(|e| {
        format!(
            "copy lsp_mock_server from {} to {}: {e}",
            built.display(),
            tmp.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmp, perms)?;
    }
    let _ = fs::remove_file(dest);
    fs::rename(&tmp, dest)
        .map_err(|e| format!("rename lsp_mock_server into {}: {e}", dest.display()))?;
    if !is_real_elf(dest) {
        return Err(format!("missing ELF after copy to {}", dest.display()).into());
    }
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
