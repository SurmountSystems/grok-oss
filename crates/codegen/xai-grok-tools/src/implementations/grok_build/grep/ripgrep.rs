use std::path::PathBuf;
use std::sync::OnceLock;

#[cfg(bundle_rg)]
const RG_BYTES: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle-rg/rg-",
    env!("GROK_TOOLS_RG_VER"),
    "-",
    env!("GROK_TOOLS_RG_TARGET"),
    ".bin"
));

#[cfg(bundle_rg)]
fn resolve_bundled_rg() -> std::io::Result<PathBuf> {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    let p = crate::util::grok_home().join("vendor").join(concat!(
        "rg-",
        env!("GROK_TOOLS_RG_VER"),
        "-",
        env!("GROK_TOOLS_RG_TARGET")
    ));
    if !p.exists() {
        fs::create_dir_all(p.parent().unwrap())?;
        fs::write(&p, RG_BYTES)?;
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&p)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&p, perms)?;
        }
    }
    Ok(p)
}

/// Get the path to the ripgrep executable.
///
/// In release builds with bundling enabled, this extracts the bundled ripgrep
/// binary to ~/.grok/vendor/ and returns that path.
/// Otherwise, assumes `rg` is in PATH.
pub fn rg_path() -> PathBuf {
    static RG_EXEC: OnceLock<PathBuf> = OnceLock::new();
    RG_EXEC
        .get_or_init(|| {
            #[cfg(bundle_rg)]
            {
                resolve_bundled_rg().unwrap_or_else(|_| PathBuf::from("rg"))
            }
            #[cfg(not(bundle_rg))]
            {
                // RG_BIN_PATH: explicit override (tests / packaging can set this).
                if let Ok(p) = std::env::var("RG_BIN_PATH") {
                    return PathBuf::from(p);
                }
                // Some hermetic test runners set RUNFILES_DIR and ship rg as a
                // data dependency rather than on PATH. Scan for a directory
                // entry containing "ripgrep_hermetic" and prefer arch-scoped
                // paths when present.
                if let Ok(rf) = std::env::var("RUNFILES_DIR") {
                    let base = PathBuf::from(rf);
                    if let Ok(entries) = std::fs::read_dir(&base) {
                        for entry in entries.flatten() {
                            let name = entry.file_name();
                            if name.to_string_lossy().contains("ripgrep_hermetic") {
                                for sub in ["amd64/rg", "arm64/rg", "rg"] {
                                    let candidate = entry.path().join(sub);
                                    if candidate.exists() {
                                        return candidate;
                                    }
                                }
                            }
                        }
                    }
                }
                PathBuf::from("rg")
            }
        })
        .clone()
}

#[cfg(test)]
mod bundle_source_contract {
    #[test]
    fn bundled_rg_is_cargo_built_from_the_ripgrep_crate_github_musl_tarball_is_not_the_install_path()
     {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs"));
        assert!(
            src.contains("cargo")
                && src.contains("install")
                && src.contains("ripgrep")
                && src.contains("--bin")
                && src.contains("rg"),
            "bundled rg is cargo-built from the ripgrep crate; GitHub musl tarball is not the install path"
        );
        assert!(
            !src.contains("github.com/BurntSushi/ripgrep/releases")
                && !src.contains("ripgrep/releases/download"),
            "xai-grok-tools build.rs must not download a GitHub musl ripgrep tarball"
        );
        assert!(
            !src.contains("GROK_TOOLS_RG_TARGET=x86_64-unknown-linux-musl"),
            "xai-grok-tools build.rs must not default GROK_TOOLS_RG_TARGET to musl"
        );
    }

    #[test]
    fn bundled_fd_is_cargo_built_from_the_fd_find_crate_github_musl_tarball_is_not_the_install_path()
     {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs"));
        assert!(
            src.contains("cargo")
                && src.contains("install")
                && src.contains("fd-find")
                && src.contains("--bin")
                && src.contains("fd"),
            "bundled fd is cargo-built from the fd-find crate; GitHub musl tarball is not the install path"
        );
        assert!(
            !src.contains("github.com/sharkdp/fd/releases")
                && !src.contains("fd/releases/download"),
            "xai-grok-tools build.rs must not download a GitHub musl fd tarball"
        );
        assert!(
            !src.contains("GROK_TOOLS_FD_TARGET=x86_64-unknown-linux-musl")
                && !src.contains("x86_64-unknown-linux-musl"),
            "xai-grok-tools build.rs must not default GROK_TOOLS_FD_TARGET to musl"
        );
    }
}
