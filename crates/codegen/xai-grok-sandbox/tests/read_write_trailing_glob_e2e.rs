//! E2E: a `read_write` config entry with a trailing `/**` must grant the
//! parent directory — not create and grant a directory literally named `**`.
//!
//! The subprocess applies a custom profile whose only extra grant is
//! `<cache>/**`, then probes writes inside the cache tree (must succeed) and
//! outside any grant (must fail, proving enforcement was active).
//!
//! Soft-skips when kernel enforcement is unavailable;
//! `SANDBOX_E2E_REQUIRE_ENFORCEMENT=1` hard-requires it.
//!
//! ```text
//! cargo test -p xai-grok-sandbox --test read_write_trailing_glob_e2e -- --nocapture
//! ```

#![cfg(all(unix, feature = "enforce"))]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const ROOT_ENV: &str = "STARSTAR_E2E_ROOT";
const REQUIRE_ENV: &str = "SANDBOX_E2E_REQUIRE_ENFORCEMENT";

/// Removes the fixture tree on both pass and panic, so failed runs don't
/// accumulate under the real `~/.cache`.
struct CleanupGuard(PathBuf);

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn skip_if_enforcement_unavailable() -> bool {
    let support = xai_grok_sandbox::SandboxManager::support_info();
    if !support.is_supported {
        assert!(
            std::env::var(REQUIRE_ENV).is_err(),
            "enforcement required but sandbox unsupported: {}",
            support.details
        );
        eprintln!("skipping: sandbox unsupported ({})", support.details);
        return true;
    }
    false
}

/// Fixture root that workspace/temp grants do not cover, so the trailing-glob
/// `read_write` entry is the only reason the cache tree is writable.
///
/// Prefer `$HOME/.cache` when that tree is writable. Nix quality sets
/// `HOME=/homeless-shelter` (not a directory) and cannot write the real home
/// either; `/dev/shm` is writable and is not in `temp_writable_paths`.
fn isolation_root_outside_temp_grants() -> PathBuf {
    let stamp = format!(
        "run-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before epoch")
            .as_millis()
    );
    let mut bases = Vec::new();
    if let Some(home) = dirs::home_dir() {
        bases.push(home.join(".cache").join("grok-starstar-e2e"));
    }
    bases.push(PathBuf::from("/dev/shm").join("grok-starstar-e2e"));
    for base in bases {
        if under_temp_grant(&base) {
            continue;
        }
        let root = base.join(&stamp);
        if fs::create_dir_all(&root).is_ok() {
            return root;
        }
    }
    panic!("create fixture dir: need a writable tree that workspace/temp grants do not cover");
}

fn under_temp_grant(path: &std::path::Path) -> bool {
    let mut roots = vec![
        std::env::temp_dir(),
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
        PathBuf::from("/build"),
    ];
    if let Some(top) = std::env::var_os("NIX_BUILD_TOP") {
        roots.push(PathBuf::from(top));
    }
    roots.iter().any(|r| path == r || path.starts_with(r))
}

#[test]
fn trailing_glob_read_write_grants_parent_directory() {
    if skip_if_enforcement_unavailable() {
        return;
    }

    // Not under TMPDIR or the test workspace: base profiles already
    // write-allow those trees, which would hide allow-path isolation.
    let root = isolation_root_outside_temp_grants();
    let _cleanup = CleanupGuard(root.clone());
    let workspace = root.join("ws");
    let home = root.join("home");
    let grok_home = root.join("grok-home");
    // Under the fixture HOME, which no base grant covers: only the explicit
    // read_write entry can make this tree writable.
    let cache = home.join("cargo-cache");
    for dir in [&workspace, &grok_home, &cache.join("registry-a1b2")] {
        fs::create_dir_all(dir).expect("create fixture dir");
    }

    let cache_abs = dunce::canonicalize(&cache).expect("canonicalize cache");
    fs::write(
        grok_home.join("sandbox.toml"),
        format!(
            "[profiles.cargo]\nextends = \"workspace\"\nread_write = [{:?}]\n",
            format!("{}/**", cache_abs.display())
        ),
    )
    .expect("write sandbox.toml");

    let exe = std::env::current_exe().expect("current_exe");
    let output = Command::new(&exe)
        .env(ROOT_ENV, root.as_os_str())
        .env("HOME", home.as_os_str())
        .env("GROK_HOME", grok_home.as_os_str())
        .arg("--ignored")
        .arg("--exact")
        .arg("--nocapture")
        .arg("subprocess_entry")
        .output()
        .expect("spawn subprocess");
    eprintln!(
        "subprocess stdout:\n{}\nsubprocess stderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "subprocess failed: {output:?}");

    let starstar = cache_abs.join("**");
    assert!(
        !starstar.exists(),
        "apply must not create a literal '**' directory at {starstar:?}"
    );
    assert!(
        cache_abs
            .join("registry-a1b2")
            .join("write-marker")
            .is_file(),
        "write under the granted cache tree must have succeeded"
    );
}

/// Applies the sandbox (irreversible), so this runs only as a subprocess of
/// the test above.
#[test]
#[ignore]
fn subprocess_entry() {
    let Ok(root) = std::env::var(ROOT_ENV) else {
        return;
    };
    let root = PathBuf::from(root);
    let workspace = root.join("ws");
    let cache = dunce::canonicalize(root.join("home").join("cargo-cache")).expect("cache");

    let profile: xai_grok_sandbox::ProfileName = "cargo".parse().expect("profile name");
    let mut mgr = xai_grok_sandbox::SandboxManager::new(profile, &workspace);
    if let Err(e) = mgr.apply(&workspace) {
        eprintln!("FAIL: apply errored: {e}");
        std::process::exit(3);
    }

    if let Err(e) = fs::write(cache.join("registry-a1b2").join("write-marker"), b"ok") {
        eprintln!("FAIL: write under granted cache tree denied: {e}");
        std::process::exit(4);
    }
    // Control probe: HOME itself carries no grant, so enforcement being
    // active is observable — without it the positive probe proves nothing.
    if fs::write(root.join("home").join("denied-marker"), b"no").is_ok() {
        eprintln!("FAIL: write outside all grants succeeded (sandbox not enforced?)");
        std::process::exit(5);
    }
    std::process::exit(0);
}
