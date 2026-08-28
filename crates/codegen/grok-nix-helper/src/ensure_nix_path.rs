//! Prefer a working `nix` binary on PATH.
//!
//! Host package-manager installs can fail hard while a store or profile copy
//! still works. Honor `NIX_BIN` without probing. Skip `/usr/bin/nix` (and
//! `/bin/nix`, `/usr/local/bin/nix`) on the first pass. Do not treat a
//! `/bin/true` named nix as working (`--version` must print `Nix`).

use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnsureError {
    NixBinNotExecutable(PathBuf),
    NoneFound,
}

impl EnsureError {
    fn message(&self) -> String {
        match self {
            EnsureError::NixBinNotExecutable(p) => {
                format!(
                    "grok-nix-helper ensure-nix-path: NIX_BIN is not executable: {}",
                    p.display()
                )
            }
            EnsureError::NoneFound => {
                "grok-nix-helper ensure-nix-path: no working nix found\n  Set NIX_BIN to a working binary, or repair the host nix install.".into()
            }
        }
    }
}

pub fn is_system_nix(path: &Path) -> bool {
    matches!(
        path.to_str(),
        Some("/usr/bin/nix") | Some("/bin/nix") | Some("/usr/local/bin/nix")
    )
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// True when `bin --version` prints `Nix` within two seconds.
pub fn probe_nix(bin: &Path, allow_system: bool) -> bool {
    if !allow_system && is_system_nix(bin) {
        return false;
    }
    if !is_executable(bin) {
        return false;
    }
    let mut child = match Command::new(bin)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() >= PROBE_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return false,
        }
    }
    let mut buf = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut buf);
    }
    buf.contains("Nix")
}

fn find_in_path(name: &str, path: &str) -> Option<PathBuf> {
    for dir in path.split(':').filter(|d| !d.is_empty()) {
        let cand = Path::new(dir).join(name);
        if is_executable(&cand) {
            return Some(cand);
        }
    }
    None
}

fn parse_nix_semver(path: &Path) -> Option<(u32, u32, u32)> {
    let s = path.to_str()?;
    let idx = s.find("-nix-")?;
    let rest = s[idx + 5..].split('/').next()?;
    let mut nums = rest.split('.').filter_map(|p| {
        let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            None
        } else {
            digits.parse::<u32>().ok()
        }
    });
    Some((
        nums.next().unwrap_or(0),
        nums.next().unwrap_or(0),
        nums.next().unwrap_or(0),
    ))
}

fn pick_store_nix() -> Option<PathBuf> {
    let rd = fs::read_dir("/nix/store").ok()?;
    let mut cands: Vec<(PathBuf, (u32, u32, u32))> = Vec::new();
    for ent in rd.flatten() {
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if let Some(rest) = name.split_once("-nix-") {
            let ver = rest.1;
            if ver.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                let p = ent.path().join("bin/nix");
                if is_executable(&p) {
                    let v = parse_nix_semver(&p).unwrap_or((0, 0, 0));
                    cands.push((p, v));
                }
            }
        }
    }
    cands.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));
    cands
        .into_iter()
        .map(|(p, _)| p)
        .find(|p| probe_nix(p, false))
}

fn dir_of(bin: &Path) -> PathBuf {
    bin.parent()
        .map(Path::to_path_buf)
        .or_else(|| {
            fs::canonicalize(bin)
                .ok()
                .and_then(|p| p.parent().map(Path::to_path_buf))
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Prepend `dir` to PATH if it is not already a component.
pub fn prepend_path(dir: &Path, path: &str) -> String {
    let dir_s = dir.to_string_lossy();
    if path.split(':').any(|p| p == dir_s) {
        path.to_string()
    } else if path.is_empty() {
        dir_s.into_owned()
    } else {
        format!("{dir_s}:{path}")
    }
}

/// Resolve a PATH that prefers a working nix. Does not mutate the process env.
pub fn resolve_path(
    nix_bin: Option<&Path>,
    path: &str,
) -> Result<(String, Option<PathBuf>), EnsureError> {
    if let Some(bin) = nix_bin {
        if !is_executable(bin) {
            return Err(EnsureError::NixBinNotExecutable(bin.to_path_buf()));
        }
        let dir = dir_of(bin);
        return Ok((prepend_path(&dir, path), Some(bin.to_path_buf())));
    }

    if let Some(bin) = find_in_path("nix", path)
        && probe_nix(&bin, false)
    {
        return Ok((path.to_string(), Some(bin)));
    }

    if let Some(picked) = pick_store_nix() {
        let dir = dir_of(&picked);
        let _ = writeln!(
            io::stderr(),
            "==> grok-nix-helper ensure-nix-path: using {}",
            picked.display()
        );
        return Ok((prepend_path(&dir, path), Some(picked)));
    }

    let mut sys_cands: Vec<PathBuf> = Vec::new();
    if let Some(bin) = find_in_path("nix", path) {
        sys_cands.push(bin);
    }
    sys_cands.extend([
        PathBuf::from("/usr/bin/nix"),
        PathBuf::from("/bin/nix"),
        PathBuf::from("/usr/local/bin/nix"),
    ]);
    for sys in sys_cands {
        if !is_executable(&sys) {
            continue;
        }
        if !is_system_nix(&sys) {
            continue;
        }
        if probe_nix(&sys, true) {
            let dir = dir_of(&sys);
            let _ = writeln!(
                io::stderr(),
                "==> grok-nix-helper ensure-nix-path: using {}",
                sys.display()
            );
            return Ok((prepend_path(&dir, path), Some(sys)));
        }
    }

    Err(EnsureError::NoneFound)
}

fn posix_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn run(args: &[String]) -> ExitCode {
    let export = args.iter().any(|a| a == "--export");
    let nix_bin = env::var_os("NIX_BIN").map(PathBuf::from);
    let path = env::var("PATH").unwrap_or_default();
    match resolve_path(nix_bin.as_deref(), &path) {
        Ok((new_path, picked)) => {
            if export {
                println!("export PATH={}", posix_single_quote(&new_path));
                return ExitCode::SUCCESS;
            }
            if let Some(p) = picked {
                let _ = writeln!(io::stdout(), "{}", p.display());
            } else if let Some(p) = find_in_path("nix", &new_path) {
                let _ = writeln!(io::stdout(), "{}", p.display());
            }
            let mut ver = Command::new("nix");
            ver.arg("--version");
            ver.env("PATH", &new_path);
            match ver.status() {
                Ok(st) if st.success() => ExitCode::SUCCESS,
                Ok(st) => ExitCode::from(st.code().unwrap_or(1) as u8),
                Err(e) => {
                    let _ = writeln!(io::stderr(), "grok-nix-helper ensure-nix-path: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Err(e) => {
            let _ = writeln!(io::stderr(), "{}", e.message());
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_package_paths() {
        assert!(is_system_nix(Path::new("/usr/bin/nix")));
        assert!(is_system_nix(Path::new("/bin/nix")));
        assert!(is_system_nix(Path::new("/usr/local/bin/nix")));
        assert!(!is_system_nix(Path::new(
            "/nix/store/abc-nix-2.24.0/bin/nix"
        )));
    }

    #[test]
    fn probe_rejects_true_named_nix() {
        let true_bin = Path::new("/bin/true");
        if true_bin.is_file() {
            assert!(
                !probe_nix(true_bin, true),
                "/bin/true --version must not count as Nix"
            );
        }
    }

    #[test]
    fn nix_bin_not_executable() {
        let missing = Path::new("/no/such/nix-bin-helper-test");
        let err = resolve_path(Some(missing), "/usr/bin").unwrap_err();
        assert!(matches!(err, EnsureError::NixBinNotExecutable(_)));
    }

    #[test]
    fn prepend_does_not_duplicate() {
        let dir = Path::new("/nix/store/zz-nix/bin");
        let path = "/nix/store/zz-nix/bin:/usr/bin";
        assert_eq!(prepend_path(dir, path), path);
        assert_eq!(
            prepend_path(dir, "/usr/bin"),
            "/nix/store/zz-nix/bin:/usr/bin"
        );
    }

    #[test]
    fn posix_quote_escapes_single_quote() {
        assert_eq!(posix_single_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn parse_store_semver() {
        let p = Path::new("/nix/store/hash-nix-2.24.12/bin/nix");
        assert_eq!(parse_nix_semver(p), Some((2, 24, 12)));
    }
}
