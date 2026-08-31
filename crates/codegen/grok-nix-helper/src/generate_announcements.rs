//! Copy ts-rs announcement bindings into the desktop generated-types dir.
//!
//! Pipeline: cargo test --features ts, copy with a do-not-edit header, oxfmt.
//! Never eval. SHA-1 is not used.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

pub const GENERATED_HEADER: &str = "// generated. Do not edit by hand.\n// Regenerate via grok-nix-helper generate-announcements.\n";

pub fn with_header(body: &str) -> String {
    format!("{GENERATED_HEADER}{body}")
}

/// Copy exported `.ts` files from `src` into `dst`. Returns count copied.
pub fn copy_bindings(src: &Path, dst: &Path) -> Result<usize, String> {
    let rd = fs::read_dir(src).map_err(|e| format!("[generate] read {src:?}: {e}"))?;
    let mut files: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ts"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!(
            "[generate] no bindings exported — aborting before touching {}",
            dst.display()
        ));
    }
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    if let Ok(existing) = fs::read_dir(dst) {
        for e in existing.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("ts") {
                let _ = fs::remove_file(&p);
            }
        }
    }
    for f in &files {
        let name = f
            .file_name()
            .ok_or_else(|| "binding has no name".to_string())?;
        let body = fs::read_to_string(f).map_err(|e| e.to_string())?;
        fs::write(dst.join(name), with_header(&body)).map_err(|e| e.to_string())?;
    }
    Ok(files.len())
}

fn default_crate_dir(repo: &Path) -> PathBuf {
    repo.join("crates/codegen/xai-grok-announcements")
}

fn default_dest(repo: &Path) -> PathBuf {
    repo.join("frontend/apps/grok-desktop/src/acp/generated")
}

pub fn run(args: &[String]) -> ExitCode {
    let repo = crate::git_cmd::find_repo_root();
    let crate_dir = args
        .iter()
        .position(|a| a == "--crate-dir")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| default_crate_dir(&repo));
    let dest = args
        .iter()
        .position(|a| a == "--dest")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| default_dest(&repo));

    let tmp = env_temp("grok-announcements-ts");
    if let Err(e) = fs::create_dir_all(&tmp) {
        let _ = writeln!(io::stderr(), "[generate] temp dir: {e}");
        return ExitCode::from(1);
    }

    println!("[generate] 1/3 exporting ts-rs bindings (cargo test --features ts) …");
    let status = Command::new("cargo")
        .current_dir(&crate_dir)
        .args(["test", "--quiet", "--features", "ts"])
        .env("TS_RS_EXPORT_DIR", &tmp)
        .status();
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            let _ = fs::remove_dir_all(&tmp);
            let _ = writeln!(
                io::stderr(),
                "[generate] cargo test --features ts failed (exit {})",
                s.code().unwrap_or(1)
            );
            return ExitCode::from(s.code().unwrap_or(1) as u8);
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            let _ = writeln!(io::stderr(), "[generate] cargo: {e}");
            return ExitCode::from(1);
        }
    }

    println!("[generate] 2/3 copying bindings -> {}", dest.display());
    let n = match copy_bindings(&tmp, &dest) {
        Ok(n) => n,
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            let _ = writeln!(io::stderr(), "{e}");
            return ExitCode::from(1);
        }
    };

    println!("[generate] 3/3 formatting (oxfmt) …");
    let desktop = dest
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap_or(&dest);
    let _ = Command::new("pnpm")
        .current_dir(desktop)
        .args(["exec", "oxfmt", "--write", "src/acp/generated"])
        .status();
    let _ = fs::remove_dir_all(&tmp);
    println!("[generate] done — {n} bindings.");
    ExitCode::SUCCESS
}

fn env_temp(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("{prefix}-{}", std::process::id()));
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_names_helper_not_generate_sh() {
        assert!(GENERATED_HEADER.contains("grok-nix-helper generate-announcements"));
        assert!(!GENERATED_HEADER.contains("generate.sh"));
        let out = with_header("export type Foo = string;\n");
        assert!(out.starts_with("// generated"));
        assert!(out.contains("export type Foo"));
    }

    #[test]
    fn copy_aborts_when_export_dir_empty() {
        let tmp = env_temp("grok-ann-empty");
        let dst = env_temp("grok-ann-dst");
        let _ = fs::remove_dir_all(&tmp);
        let _ = fs::remove_dir_all(&dst);
        fs::create_dir_all(&tmp).unwrap();
        let err = copy_bindings(&tmp, &dst).unwrap_err();
        assert!(err.contains("no bindings exported"));
        assert!(!dst.join("Foo.ts").exists());
        let _ = fs::remove_dir_all(&tmp);
        let _ = fs::remove_dir_all(&dst);
    }

    #[test]
    fn copy_writes_header_and_clears_old_ts() {
        let src = env_temp("grok-ann-src");
        let dst = env_temp("grok-ann-out");
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("Foo.ts"), "export type Foo = number;\n").unwrap();
        fs::write(dst.join("Old.ts"), "stale\n").unwrap();
        let n = copy_bindings(&src, &dst).unwrap();
        assert_eq!(n, 1);
        let body = fs::read_to_string(dst.join("Foo.ts")).unwrap();
        assert!(body.starts_with(GENERATED_HEADER) || body.contains("Do not edit by hand"));
        assert!(body.contains("export type Foo"));
        assert!(!dst.join("Old.ts").exists());
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }
}
