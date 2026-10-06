//! Link the system DuckDB shared library, or fail the build.
//!
//! `DUCKDB_LIB_DIR` and `DUCKDB_INCLUDE_DIR` are the libduckdb-sys names.
//! pkg-config's module name is `duckdb`. This file does not compile
//! `duckdb.cpp` and does not link a static archive.

use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DUCKDB_LIB_DIR");
    println!("cargo:rerun-if-env-changed=DUCKDB_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=DUCKDB_STATIC");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");

    if let Some(value) = nonempty("DUCKDB_STATIC") {
        if value != "0" {
            fail(&format!(
                "DUCKDB_STATIC={value} requests a static link. Unset it."
            ));
        }
    }

    let shared = shared_filename();
    if shared.is_empty() {
        fail("this target has no DuckDB shared library name.");
    }

    let (lib_dir, include_dir) = match (nonempty("DUCKDB_LIB_DIR"), nonempty("DUCKDB_INCLUDE_DIR"))
    {
        (Some(lib), Some(include)) => {
            let lib_dir = PathBuf::from(lib);
            let include_dir = PathBuf::from(include);
            require_pair(&lib_dir, &include_dir);
            (lib_dir, include_dir)
        }
        (Some(_), None) | (None, Some(_)) => fail(
            "DUCKDB_LIB_DIR and DUCKDB_INCLUDE_DIR must both be set. One without the other is not a search of another library.",
        ),
        (None, None) => locate_unconfigured(),
    };

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=duckdb");
    println!("cargo:include={}", include_dir.display());
}

fn locate_unconfigured() -> (PathBuf, PathBuf) {
    if let Some((lib_dir, include_dir)) = pkg_config_pair() {
        require_pair(&lib_dir, &include_dir);
        return (lib_dir, include_dir);
    }
    if let Some(pair) = system_pair() {
        return pair;
    }
    if let Some(path) = find_static_archive() {
        fail(&format!(
            "Found a static archive at {} and no usable {}.",
            path.display(),
            shared_filename()
        ));
    }
    fail(&format!("{} or duckdb.h is missing.", shared_filename()));
}

fn pkg_config_pair() -> Option<(PathBuf, PathBuf)> {
    let output = Command::new("pkg-config")
        .args(["--libs-only-L", "--cflags-only-I", "duckdb"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).unwrap_or_default();
    let mut lib_dir = None;
    let mut include_dir = None;
    for token in text.split_whitespace() {
        if let Some(path) = token.strip_prefix("-L") {
            if !path.is_empty() {
                lib_dir = Some(PathBuf::from(path));
            }
        } else if let Some(path) = token.strip_prefix("-I") {
            if !path.is_empty() {
                include_dir = Some(PathBuf::from(path));
            }
        }
    }
    match (lib_dir, include_dir) {
        (Some(lib_dir), Some(include_dir)) => Some((lib_dir, include_dir)),
        _ => fail(
            "pkg-config duckdb did not report both a library directory and an include directory.",
        ),
    }
}

fn system_pair() -> Option<(PathBuf, PathBuf)> {
    let lib_dir = system_lib_dirs().into_iter().find(|dir| {
        let shared = dir.join(shared_filename());
        shared.is_file() && is_shared_library(&shared)
    })?;
    let include_dir = system_include_dirs()
        .into_iter()
        .find(|dir| dir.join("duckdb.h").is_file())?;
    Some((lib_dir, include_dir))
}

fn require_pair(lib_dir: &Path, include_dir: &Path) {
    let shared = lib_dir.join(shared_filename());
    let header = include_dir.join("duckdb.h");
    let shared_ok = shared.is_file() && is_shared_library(&shared);
    let header_ok = header.is_file();
    if shared_ok && header_ok {
        return;
    }
    if !shared_ok {
        if is_ar(&shared) {
            fail(&format!(
                "Found a static archive at {} and no usable {}.",
                shared.display(),
                shared_filename()
            ));
        }
        if let Some(path) = static_archive_in(lib_dir) {
            fail(&format!(
                "Found a static archive at {} and no usable {}.",
                path.display(),
                shared_filename()
            ));
        }
    }
    fail(&format!(
        "{} or duckdb.h is missing (looked in {} and {}).",
        shared_filename(),
        lib_dir.display(),
        include_dir.display()
    ));
}

fn find_static_archive() -> Option<PathBuf> {
    system_lib_dirs()
        .into_iter()
        .find_map(|dir| static_archive_in(&dir))
}

fn static_archive_in(dir: &Path) -> Option<PathBuf> {
    let archive = dir.join("libduckdb.a");
    if archive.is_file() {
        return Some(archive);
    }
    None
}

fn nonempty(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn target_os() -> String {
    env::var("CARGO_CFG_TARGET_OS").unwrap_or_default()
}

fn shared_filename() -> &'static str {
    match target_os().as_str() {
        "linux" => "libduckdb.so",
        "macos" => "libduckdb.dylib",
        "windows" => "duckdb.dll",
        _ => "",
    }
}

fn system_lib_dirs() -> Vec<PathBuf> {
    match target_os().as_str() {
        "linux" => vec![
            PathBuf::from("/usr/lib"),
            PathBuf::from("/usr/lib64"),
            PathBuf::from("/usr/local/lib"),
            PathBuf::from("/usr/local/lib64"),
            PathBuf::from("/lib"),
            PathBuf::from("/lib64"),
        ],
        "macos" => vec![
            PathBuf::from("/usr/local/lib"),
            PathBuf::from("/opt/homebrew/lib"),
        ],
        "windows" => Vec::new(),
        _ => Vec::new(),
    }
}

fn system_include_dirs() -> Vec<PathBuf> {
    match target_os().as_str() {
        "linux" => vec![
            PathBuf::from("/usr/include"),
            PathBuf::from("/usr/local/include"),
        ],
        "macos" => vec![
            PathBuf::from("/usr/local/include"),
            PathBuf::from("/opt/homebrew/include"),
        ],
        "windows" => Vec::new(),
        _ => Vec::new(),
    }
}

fn is_shared_library(path: &Path) -> bool {
    match target_os().as_str() {
        "linux" => is_elf_shared_object(path),
        "macos" => path.is_file() && !is_ar(path),
        "windows" => is_mz(path),
        _ => false,
    }
}

fn is_elf_shared_object(path: &Path) -> bool {
    let mut header = [0u8; 20];
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    if file.read(&mut header).ok() != Some(header.len()) {
        return false;
    }
    if &header[0..4] != b"\x7fELF" {
        return false;
    }
    // e_type follows the 16-byte e_ident. ET_DYN is 3.
    let kind = match header[5] {
        1 => u16::from_le_bytes([header[16], header[17]]),
        2 => u16::from_be_bytes([header[16], header[17]]),
        _ => return false,
    };
    kind == 3
}

fn is_mz(path: &Path) -> bool {
    let mut header = [0u8; 2];
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    file.read(&mut header).ok() == Some(2) && &header == b"MZ"
}

fn is_ar(path: &Path) -> bool {
    let mut header = [0u8; 8];
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    file.read(&mut header).ok() == Some(8) && &header == b"!<arch>\n"
}

fn fail(message: &str) -> ! {
    panic!(
        "DuckDB shared library is required. {message} This build does not compile duckdb.cpp and does not link a static archive."
    );
}
