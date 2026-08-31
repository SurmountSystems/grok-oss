//! Extract DWARF/debug symbols to a sidecar, strip the binary, add GNU debuglink.
//!
//! Linux: objcopy or llvm-objcopy. macOS: dsymutil + strip.

use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugPlan {
    Linux {
        objcopy: PathBuf,
        debug: PathBuf,
        bin: PathBuf,
        base: String,
        dir: PathBuf,
    },
    Darwin {
        bin: PathBuf,
        dsym: PathBuf,
    },
}

pub fn find_objcopy(path: &str) -> Option<PathBuf> {
    for name in ["objcopy", "llvm-objcopy"] {
        for dir in path.split(':').filter(|d| !d.is_empty()) {
            let cand = Path::new(dir).join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

pub fn plan_for(os: &str, bin: &Path, path: &str) -> Result<DebugPlan, String> {
    if !bin.is_file() {
        return Err(format!(
            "extract-debug-sidecar: binary not found: {}",
            bin.display()
        ));
    }
    let bin = bin.canonicalize().unwrap_or_else(|_| bin.to_path_buf());
    let dir = bin.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    let base = bin
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match os {
        "linux" => {
            let objcopy = find_objcopy(path).ok_or_else(|| {
                "extract-debug-sidecar: need objcopy or llvm-objcopy (binutils / llvm)".to_string()
            })?;
            let debug = PathBuf::from(format!("{}.debug", bin.display()));
            Ok(DebugPlan::Linux {
                objcopy,
                debug,
                bin,
                base,
                dir,
            })
        }
        "macos" | "darwin" => Ok(DebugPlan::Darwin {
            dsym: PathBuf::from(format!("{}.dSYM", bin.display())),
            bin,
        }),
        other => Err(format!(
            "extract-debug-sidecar: unsupported OS '{other}' (Linux + macOS only)"
        )),
    }
}

fn run_argv(cmd: &mut Command) -> Result<(), String> {
    let status = cmd.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "extract-debug-sidecar: command failed: {:?}",
            cmd.get_program()
        ))
    }
}

pub fn execute(plan: &DebugPlan) -> Result<(), String> {
    match plan {
        DebugPlan::Linux {
            objcopy,
            debug,
            bin,
            base,
            dir,
        } => {
            println!("==> extract debug → {}", debug.display());
            run_argv(
                Command::new(objcopy)
                    .args(["--only-keep-debug"])
                    .arg(bin)
                    .arg(debug),
            )?;
            println!("==> strip debug + unneeded from {}", bin.display());
            run_argv(
                Command::new(objcopy)
                    .args(["--strip-debug", "--strip-unneeded"])
                    .arg(bin),
            )?;
            println!("==> add GNU debuglink → {base}.debug");
            run_argv(
                Command::new(objcopy)
                    .current_dir(dir)
                    .arg(format!("--add-gnu-debuglink={base}.debug"))
                    .arg(base),
            )?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(debug) {
                    let mut perm = meta.permissions();
                    perm.set_mode(perm.mode() & !0o111);
                    let _ = std::fs::set_permissions(debug, perm);
                }
            }
            println!(
                "==> done: {} (stripped) + {}",
                bin.display(),
                debug.display()
            );
            Ok(())
        }
        DebugPlan::Darwin { bin, dsym } => {
            println!("==> dsymutil → {}", dsym.display());
            run_argv(Command::new("dsymutil").arg(bin).arg("-o").arg(dsym))?;
            println!("==> strip {}", bin.display());
            run_argv(Command::new("strip").arg("-S").arg(bin))?;
            println!(
                "==> done: {} (stripped) + {}",
                bin.display(),
                dsym.display()
            );
            Ok(())
        }
    }
}

pub fn run(args: &[String]) -> ExitCode {
    if args.len() != 1 {
        let _ = writeln!(
            io::stderr(),
            "Usage: grok-nix-helper extract-debug-sidecar <path-to-binary>"
        );
        return ExitCode::from(2);
    }
    let bin = PathBuf::from(&args[0]);
    let os = env::consts::OS;
    let path = env::var("PATH").unwrap_or_default();
    match plan_for(os, &bin, &path) {
        Ok(plan) => match execute(&plan) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                let _ = writeln!(io::stderr(), "{e}");
                ExitCode::from(1)
            }
        },
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            if e.contains("not found") {
                ExitCode::from(1)
            } else if e.contains("Usage") {
                ExitCode::from(2)
            } else {
                ExitCode::from(1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_binary_is_named() {
        let err = plan_for(
            "linux",
            Path::new("/no/such/grok-oss-debug-bin"),
            "/usr/bin",
        )
        .unwrap_err();
        assert!(err.contains("binary not found"));
        assert!(err.contains("/no/such/grok-oss-debug-bin"));
    }

    #[test]
    fn unsupported_os_names_linux_and_macos() {
        let tmp = env::temp_dir().join(format!("grok-extract-debug-{}", std::process::id()));
        let _ = std::fs::write(&tmp, b"x");
        let err = plan_for("windows", &tmp, "/usr/bin").unwrap_err();
        assert!(err.contains("unsupported OS"));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn linux_plan_uses_objcopy_when_present() {
        let tmp = env::temp_dir().join(format!("grok-extract-debug-bin-{}", std::process::id()));
        let _ = std::fs::write(&tmp, b"x");
        let bindir = env::temp_dir().join(format!("grok-extract-objcopy-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&bindir);
        let obj = bindir.join("objcopy");
        let _ = std::fs::write(&obj, b"#!/bin/true\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&obj) {
                let mut p = meta.permissions();
                p.set_mode(0o755);
                let _ = std::fs::set_permissions(&obj, p);
            }
        }
        let path = format!("{}:/usr/bin", bindir.display());
        let plan = plan_for("linux", &tmp, &path).expect("plan");
        match plan {
            DebugPlan::Linux { objcopy, debug, .. } => {
                assert_eq!(objcopy, obj);
                assert!(debug.to_string_lossy().ends_with(".debug"));
            }
            DebugPlan::Darwin { .. } => panic!("expected linux plan"),
        }
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(&obj);
        let _ = std::fs::remove_dir(&bindir);
    }
}
