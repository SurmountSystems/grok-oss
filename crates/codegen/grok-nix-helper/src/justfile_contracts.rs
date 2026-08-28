//! Source contracts that used to live as justfile `#!/usr/bin/env bash`
//! recipes that only grepped justfile/flake. Isolated crate tests skip when
//! the repo root is not next to this crate (crane helperSrc). Quality
//! workspace nextest from the full tree proves these.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> Option<PathBuf> {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..8 {
        if p.join("justfile").is_file() && p.join("flake.nix").is_file() {
            return Some(p);
        }
        if !p.pop() {
            break;
        }
    }
    None
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

fn recipe_starts(line: &str, name: &str) -> bool {
    let t = line.trim_end();
    let Some(rest) = t.strip_prefix(name) else {
        return false;
    };
    rest.is_empty()
        || rest.starts_with(':')
        || rest.starts_with(' ')
        || rest.starts_with('\t')
        || rest.starts_with('*')
        || rest.starts_with('+')
}

fn looks_like_recipe_header(line: &str) -> bool {
    let t = line.trim_end();
    if t.is_empty() || t.starts_with('#') || t.starts_with('[') {
        return false;
    }
    if t.starts_with(' ') || t.starts_with('\t') {
        return false;
    }
    let name = t.split([':', ' ', '\t']).next().unwrap_or("");
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        && t.contains(':')
}

fn recipe_body(justfile: &str, name: &str) -> String {
    let mut out = String::new();
    let mut taking = false;
    for line in justfile.lines() {
        if taking {
            if looks_like_recipe_header(line) {
                break;
            }
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if recipe_starts(line, name) {
            taking = true;
            out.push_str(line);
            out.push('\n');
        }
    }
    assert!(taking, "justfile must define recipe {name}");
    out
}

fn quality_src(root: &Path) -> String {
    let mut s = read(root, "flake.nix");
    s.push('\n');
    s.push_str(&read(root, "flake/workspace-quality.nix"));
    s.push('\n');
    s.push_str(&read(root, "flake/workspace-named-test.nix"));
    s
}

fn quality_gate_build_phase(quality_nix: &str) -> &str {
    let quality_block = quality_nix
        .split("workspace-cargo-quality = craneLib.mkCargoDerivation")
        .nth(1)
        .expect("flake/workspace-quality.nix must define workspace-cargo-quality");
    let start = quality_block
        .find("buildPhaseCargoCommand = ''")
        .expect("workspace-cargo-quality must set buildPhaseCargoCommand");
    let rest = &quality_block[start..];
    let end = rest
        .find("'';")
        .expect("workspace-cargo-quality buildPhaseCargoCommand must close");
    &rest[..end]
}

fn must_contain_at(hay: &str, needle: &str, what: &str) -> usize {
    hay.find(needle)
        .unwrap_or_else(|| panic!("quality buildPhase must include {what}: {needle}\n{hay}"))
}

fn skip_or_root() -> Option<PathBuf> {
    match repo_root() {
        Some(r) => Some(r),
        None => {
            eprintln!(
                "skipping justfile contracts: repo root not next to crate (isolated helperSrc)"
            );
            None
        }
    }
}

fn toml_bracket_list<'a>(src: &'a str, key: &str) -> &'a str {
    let start = src
        .find(key)
        .unwrap_or_else(|| panic!("Cargo.toml must contain {key}"));
    let rest = &src[start..];
    let open = rest
        .find('[')
        .unwrap_or_else(|| panic!("{key} must be a list"));
    let close = rest[open..]
        .find(']')
        .unwrap_or_else(|| panic!("{key} list must close"));
    &rest[open..=open + close]
}

#[test]
fn grep_only_justfile_contract_recipes_are_gone() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    for name in [
        "test-check-remote-quotes-quality-attr:",
        "test-test-remote-is-force-remote-nix:",
        "test-test-remote-runs-tests-not-no-run:",
        "test-check-remote-exports-nix-sshopts:",
    ] {
        assert!(
            !just.contains(name),
            "grep-only just recipe {name} must move into grok-nix-helper tests"
        );
        let extra = just
            .lines()
            .find(|l| l.starts_with("test-extra:"))
            .unwrap_or("");
        let slug = name.trim_end_matches(':');
        assert!(
            !extra.contains(slug),
            "test-extra must not list deleted grep-only recipe {slug}"
        );
    }
}

#[test]
fn just_test_clippy_lints_all_targets() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let clippy = recipe_body(&just, "test-clippy");
    assert!(
        clippy.contains("cargo clippy --workspace --all-targets --locked -- -D warnings"),
        "just test-clippy must lint --all-targets:\n{clippy}"
    );
    assert!(
        !clippy.contains("--manifest-path crates/codegen/cargo-mem-guard/Cargo.toml")
            && !clippy.contains("--manifest-path crates/codegen/grok-nix-helper/Cargo.toml"),
        "just test-clippy must not clippy members via --manifest-path; --workspace covers them:\n{clippy}"
    );
    assert!(
        !(clippy.contains("--lib --bins") && !clippy.contains("--all-targets")),
        "just test-clippy must not lint only --lib --bins:\n{clippy}"
    );
    let targets = recipe_body(&just, "test-clippy-targets");
    assert!(
        targets.contains("--all-targets"),
        "just test-clippy-targets must pass --all-targets:\n{targets}"
    );
}

#[test]
fn check_remote_quotes_quality_attr_and_nix_retry_uses_argv() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let remote = recipe_body(&just, "check-remote");
    assert!(
        remote.contains("GROK_NIX_FORCE_REMOTE=1"),
        "check-remote must set GROK_NIX_FORCE_REMOTE=1:\n{remote}"
    );
    assert!(
        remote.contains("\".#workspace-cargo-quality\"")
            || remote.contains("'.#workspace-cargo-quality'"),
        "check-remote must quote .#workspace-cargo-quality:\n{remote}"
    );
    let unquoted = remote.lines().any(|l| {
        l.contains(".#workspace-cargo-quality") && !l.contains("\".#") && !l.contains("'.#")
    });
    assert!(
        !unquoted,
        "check-remote must not leave .#workspace-cargo-quality unquoted:\n{remote}"
    );
    assert!(
        !remote.contains("just ci")
            && !remote.contains("just test")
            && !remote.contains("just cargo-ci"),
        "check-remote must not run host just ci/test/cargo-ci:\n{remote}"
    );
    let retry = recipe_body(&just, "nix_retry");
    assert!(
        retry.contains("\"$@\""),
        "nix_retry must exec argv as \"$@\":\n{retry}"
    );
    let code = retry
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("grok_helper") && !code.contains("grok_nix_helper_bin"),
        "nix_retry must not trampoline through grok_helper / grok_nix_helper_bin:\n{retry}"
    );
    assert!(
        !code.contains("{{ cmd }}") && !code.contains("{{cmd}}"),
        "nix_retry must not interpolate {{{{ cmd }}}} into bash:\n{retry}"
    );
    assert!(
        !code.contains("set --"),
        "nix_retry must not set -- the machines line over \"$@\":\n{retry}"
    );
    assert!(
        just.contains("[positional-arguments]"),
        "justfile must use [positional-arguments] for argv trampolines"
    );
    let mut saw_pos = false;
    let mut found = false;
    for line in just.lines() {
        let t = line.trim();
        if t == "[positional-arguments]" {
            saw_pos = true;
            continue;
        }
        if recipe_starts(line, "nix_retry") {
            found = saw_pos;
            break;
        }
        if looks_like_recipe_header(line) {
            saw_pos = false;
        }
    }
    assert!(
        found,
        "nix_retry must set [positional-arguments] so \"$@\" is the command words"
    );
}

#[test]
fn check_remote_prints_quality_receipt_on_cache_hit() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let remote = recipe_body(&just, "check-remote");
    assert!(
        remote.contains("--print-out-paths"),
        "check-remote must print the quality store path even on a cache hit:\n{remote}"
    );
    assert!(
        remote.contains("quality-summary.txt"),
        "check-remote must cat quality-summary.txt so nextest is not silent:\n{remote}"
    );
    assert!(
        remote.contains("nix store cat"),
        "check-remote must nix store cat the receipt (not rely on -L during a cache hit):\n{remote}"
    );
    let quality = read(&root, "flake/workspace-quality.nix");
    assert!(
        quality.contains("quality-summary.txt") && quality.contains("Summary ["),
        "workspace-cargo-quality must write a nextest Summary receipt into $out"
    );
}

#[test]
fn nix_retry_flake_meta_and_check_remote_do_not_require_helper_binary() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let retry = recipe_body(&just, "nix_retry");
    let meta = recipe_body(&just, "flake-meta");
    let remote = recipe_body(&just, "check-remote");
    recipe_must_not_nix_build_helper("nix_retry", &retry);
    recipe_must_not_nix_build_helper("flake-meta", &meta);
    recipe_must_not_nix_build_helper("check-remote", &remote);
    for (name, body) in [
        ("nix_retry", retry.as_str()),
        ("flake-meta", meta.as_str()),
        ("check-remote", remote.as_str()),
    ] {
        let code = body
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("grok_helper") && !code.contains("grok_nix_helper_bin"),
            "{name} must not require grok_nix_helper_bin for flake metadata / nix_retry:\n{body}"
        );
    }
    assert!(
        meta.contains("just nix_retry"),
        "flake-meta must call just nix_retry (not a helper trampoline):\n{meta}"
    );
    assert!(
        remote.contains("just flake-meta"),
        "check-remote must reach flake metadata through just flake-meta:\n{remote}"
    );
    assert!(
        retry.contains("\"$@\"")
            && retry.contains("--cores")
            && retry.contains("64")
            && retry.contains("max-jobs")
            && retry.contains("Diff in ")
            && retry.contains("failed to start SSH connection")
            && retry.contains("ld returned 137"),
        "nix_retry live body must keep argv exec, force-remote cores, fail-fast on quality/SSH, and retry linker SIGKILL:\n{retry}"
    );
    let ld_pos = retry.find("ld returned 137").unwrap_or(usize::MAX);
    let compile_pos = retry.find("error: could not compile").unwrap_or(0);
    assert!(
        ld_pos < compile_pos,
        "nix_retry must classify ld returned 137 before error: could not compile:\n{retry}"
    );
    let extra = recipe_body(&just, "test-extra");
    assert!(
        extra.contains("test-nix-retry-does-not-require-helper-binary"),
        "test-extra must run the runtime helper-free nix_retry check:\n{extra}"
    );
    assert!(
        extra.contains("test-nix-retry-linker-sigkill-retries"),
        "test-extra must run the linker SIGKILL retry check:\n{extra}"
    );
}

#[test]
fn check_remote_exports_nix_sshopts_with_known_hosts() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let remote = recipe_body(&just, "check-remote");
    assert!(
        remote.contains("NIX_SSHOPTS"),
        "check-remote must export NIX_SSHOPTS:\n{remote}"
    );
    assert!(
        remote.contains("UserKnownHostsFile"),
        "check-remote NIX_SSHOPTS must use UserKnownHostsFile:\n{remote}"
    );
    assert!(
        remote.contains("GROK_NIX_KNOWN_HOSTS") || remote.contains(".ssh/known_hosts"),
        "check-remote must point at this account's known_hosts:\n{remote}"
    );
    assert!(
        !remote.contains("StrictHostKeyChecking=no"),
        "check-remote must not set StrictHostKeyChecking=no:\n{remote}"
    );
}

#[test]
fn github_actions_must_not_call_remote_gates() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let gha = read(&root, ".github/workflows/ci.yml");
    assert!(
        !gha.contains("check-remote"),
        "GitHub Actions must not call check-remote"
    );
    assert!(
        !gha.contains("test-remote") && !gha.contains("cargo-remote"),
        "GitHub Actions must not call test-remote or cargo-remote"
    );
    assert!(
        !gha.contains("workspace-cargo-named-test"),
        "GitHub Actions must not realize workspace-cargo-named-test"
    );
    let just = read(&root, "justfile");
    assert!(
        just.contains("check-remote:"),
        "just check-remote must stay as the backup gate"
    );
}

#[test]
fn just_ci_stays_local() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let ci = recipe_body(&just, "ci");
    assert!(
        ci.contains("just test"),
        "just ci must still run local just test:\n{ci}"
    );
    assert!(
        !ci.contains("require_remote_builder"),
        "just ci must stay local (no require_remote_builder):\n{ci}"
    );
}

#[test]
fn workspace_quality_source_matches_just_test() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let quality = quality_src(&root);
    assert!(
        quality.contains("clippy-driver"),
        "workspace-cargo-quality must lint with clippy-driver"
    );
    assert!(
        quality.contains("RUSTC_WORKSPACE_WRAPPER"),
        "quality must set RUSTC_WORKSPACE_WRAPPER to clippy-driver"
    );
    assert!(
        quality.contains("CLIPPY_ARGS"),
        "quality must set CLIPPY_ARGS so clippy-driver denies warnings"
    );
    assert!(
        quality.contains("-D__CLIPPY_HACKERY__warnings") || quality.contains("CLIPPY_ARGS="),
        "CLIPPY_ARGS must deny warnings"
    );
    assert!(
        quality.contains(
            "check --profile \"$CARGO_PROFILE\" --jobs \"$CARGO_BUILD_JOBS\" --workspace --all-targets --locked"
        ),
        "quality clippy cargo check must be --workspace --all-targets"
    );
    assert!(
        !(quality.contains("--workspace --lib --bins") && !quality.contains("--all-targets")),
        "quality must not lint only --lib --bins without --all-targets"
    );
    assert!(
        !quality.contains("cargo clippy --"),
        "quality must not invoke cargo clippy (external dispatcher)"
    );
    assert!(
        quality.contains("workspace_run_make_jobserver"),
        "quality must run clippy under workspace_run_make_jobserver"
    );
    assert!(
        quality.contains("make -j\"$CARGO_BUILD_JOBS\""),
        "jobserver helper must run make -j\"$CARGO_BUILD_JOBS\""
    );
    assert!(
        quality.contains("nextest run --workspace --locked"),
        "quality must run cargo nextest run --workspace --locked"
    );
    assert!(
        quality.contains("--build-jobs \"$CARGO_LINK_JOBS\""),
        "quality nextest compile/link must pass --build-jobs \"$CARGO_LINK_JOBS\" (not 32 parallel mold links)"
    );
    assert!(
        quality.contains("CARGO_LINK_JOBS")
            && (quality.contains("\"$linkJobs\" -gt 4") || quality.contains("linkJobs\" -gt 4")),
        "quality must cap CARGO_LINK_JOBS at 4 (nix-daemon 32GiB memory cgroup):\n{quality}"
    );
    assert!(
        !quality.contains("--no-run"),
        "workspace-cargo-quality must not be compile-only"
    );
    assert!(
        quality.contains("test --workspace --doc"),
        "quality must run cargo test --workspace --doc"
    );
    assert!(
        !quality.contains("crates/codegen/cargo-mem-guard/Cargo.toml")
            && !quality.contains("crates/codegen/grok-nix-helper/Cargo.toml"),
        "quality must not cargo check/test members via --manifest-path; --workspace covers them"
    );
    assert!(
        quality.contains("cargo-nextest"),
        "quality must put cargo-nextest on the derivation PATH"
    );
    assert!(
        quality.contains("preferLocalBuild = false"),
        "flake must set preferLocalBuild = false on the cargo quality derivation"
    );
    assert!(
        quality.contains("surmount-remote"),
        "workspace rustc must require surmount-remote"
    );
    assert!(
        quality.contains("CARGO_BUILD_JOBS = \"32\""),
        "workspace-cargo-quality must set CARGO_BUILD_JOBS = \"32\""
    );
    assert!(
        quality.contains("CARGO_PROFILE = \"dev\""),
        "workspace-cargo-quality must set CARGO_PROFILE = \"dev\""
    );
    assert!(
        quality.contains("enableParallelBuilding = true"),
        "workspace-cargo-quality must set enableParallelBuilding = true"
    );
    assert!(
        quality.contains("NIX_BUILD_CORES"),
        "cargo --jobs must be taken from NIX_BUILD_CORES"
    );
    assert!(
        quality.contains("unset MAKEFLAGS"),
        "must unset MAKEFLAGS/CARGO_MAKEFLAGS so a 1-token jobserver cannot ignore cargo --jobs"
    );
    for line in quality.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        assert!(
            !t.contains("cargo --jobs"),
            "cargo 1.97 has no global --jobs; put --jobs after the subcommand: {t}"
        );
    }
    assert!(
        !quality.contains("floor=\"${CARGO_BUILD_JOBS"),
        "do not floor cargo jobs from CARGO_BUILD_JOBS"
    );
    assert!(
        quality.contains("\"$cargoJobs\" -lt 2") || quality.contains("cargoJobs\" -lt 2"),
        "when NIX_BUILD_CORES is 1, cargo jobs must still become 32"
    );
    assert!(
        !quality.contains("-j1") && !quality.contains("--jobs 1"),
        "quality clippy/jobserver must not pass -j1 / --jobs 1"
    );
}

#[test]
fn workspace_quality_fmt_then_clippy_then_nextest_and_helper_tests() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let quality_nix = read(&root, "flake/workspace-quality.nix");
    let phase = quality_gate_build_phase(&quality_nix);
    let fmt = must_contain_at(phase, "cargo fmt --all -- --check", "fmt first");
    let wrap = must_contain_at(
        phase,
        "export RUSTC_WORKSPACE_WRAPPER=",
        "clippy-driver wrapper before clippy",
    );
    let ws_clippy = must_contain_at(
        phase,
        "cargo check --profile \"$CARGO_PROFILE\" --jobs \"$CARGO_BUILD_JOBS\" --workspace --all-targets --locked",
        "workspace clippy-as-check --all-targets",
    );
    let unwrap = must_contain_at(
        phase,
        "unset RUSTC_WORKSPACE_WRAPPER",
        "unset clippy wrapper after workspace clippy",
    );
    let nextest = must_contain_at(
        phase,
        "nextest run --workspace --locked",
        "workspace nextest after clippy",
    );
    let doctest = must_contain_at(
        phase,
        "cargo test --workspace --doc",
        "doctests after nextest",
    );
    assert!(
        !phase.contains("--manifest-path crates/codegen/cargo-mem-guard/Cargo.toml")
            && !phase.contains("--manifest-path crates/codegen/grok-nix-helper/Cargo.toml"),
        "quality must not cargo check/test members via --manifest-path; --workspace clippy and nextest cover them:\n{phase}"
    );
    assert!(
        fmt < wrap
            && wrap < ws_clippy
            && ws_clippy < unwrap
            && unwrap < nextest
            && nextest < doctest,
        "quality order must be fmt, then workspace clippy --all-targets, then nextest, then doctests; got fmt={fmt} wrap={wrap} ws={ws_clippy} unwrap={unwrap} nextest={nextest} doc={doctest}\n{phase}"
    );

    let just = read(&root, "justfile");
    let test_recipe = recipe_body(&just, "test");
    let header = test_recipe
        .lines()
        .next()
        .unwrap_or("")
        .trim_start_matches('#')
        .trim();
    assert!(
        header.contains("test: test-fmt test-clippy test-unit test-doc")
            && !header.contains("test-mem-guard")
            && !header.contains("test-grok-nix-helper"),
        "just test must stay fmt, clippy, nextest, doctests (workspace nextest covers member crate tests):\n{test_recipe}"
    );
    let clippy = recipe_body(&just, "test-clippy");
    assert!(
        clippy.contains("cargo clippy --workspace --all-targets --locked"),
        "just test-clippy must lint the workspace --all-targets:\n{clippy}"
    );
    assert!(
        !clippy.contains("--manifest-path"),
        "just test-clippy must not lint members via --manifest-path:\n{clippy}"
    );

    let named_nix = read(&root, "flake/workspace-named-test.nix");
    assert!(
        !named_nix.contains("crates/codegen/grok-nix-helper/Cargo.toml")
            && !named_nix.contains("crates/codegen/cargo-mem-guard/Cargo.toml"),
        "named-test is one cargo kind, not a late helper cargo test after workspace clippy:\n{named_nix}"
    );
}

#[test]
fn workspace_quality_deps_cargo_check_stays_locked() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let quality = read(&root, "flake/workspace-quality.nix");
    assert!(
        quality.contains(
            "cargo check --profile \"$CARGO_PROFILE\" --jobs \"$CARGO_BUILD_JOBS\" --locked --all-targets"
        ),
        "workspace-cargo-quality-deps must cargo check --locked --all-targets"
    );
    assert!(
        quality.contains(
            "cargo build --profile \"$CARGO_PROFILE\" --jobs \"$CARGO_BUILD_JOBS\" --locked"
        ),
        "workspace-cargo-quality-deps must cargo build --locked"
    );
    for line in quality.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        let is_cargo = t.contains("cargo check")
            || t.contains("cargo build")
            || t.contains("cargo test")
            || t.contains("nextest run");
        if !is_cargo {
            continue;
        }
        assert!(
            t.contains("--locked"),
            "quality cargo check/build/test/nextest must keep --locked (do not drop it to go green): {t}"
        );
    }
}

#[test]
fn workspace_root_members_include_cargo_mem_guard_and_grok_nix_helper() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let manifest = read(&root, "Cargo.toml");
    assert!(
        !manifest.contains("exclude ="),
        "workspace must not exclude crates; cargo-mem-guard and grok-nix-helper are members:\n{manifest}"
    );
    let members = toml_bracket_list(&manifest, "members =");
    assert!(
        members.contains("crates/codegen/cargo-mem-guard"),
        "cargo-mem-guard must be a workspace member:\n{members}"
    );
    assert!(
        members.contains("crates/codegen/grok-nix-helper"),
        "grok-nix-helper must be a workspace member:\n{members}"
    );
    for rel in [
        "crates/codegen/cargo-mem-guard/Cargo.toml",
        "crates/codegen/grok-nix-helper/Cargo.toml",
    ] {
        let crate_manifest = read(&root, rel);
        assert!(
            !crate_manifest.contains("[workspace]"),
            "{rel} must not declare a nested [workspace]; it is a member:\n{crate_manifest}"
        );
        assert!(
            crate_manifest.contains("edition.workspace = true"),
            "{rel} must inherit workspace.package edition:\n{crate_manifest}"
        );
        assert!(
            crate_manifest.contains("[lints]")
                && crate_manifest
                    .split("[lints]")
                    .nth(1)
                    .is_some_and(|s| s.contains("workspace = true")),
            "{rel} must inherit workspace lints:\n{crate_manifest}"
        );
    }
    let lock = read(&root, "Cargo.lock");
    assert!(
        lock.contains("name = \"grok-nix-helper\""),
        "one workspace Cargo.lock must list grok-nix-helper"
    );
    assert!(
        lock.contains("name = \"cargo-mem-guard\""),
        "one workspace Cargo.lock must list cargo-mem-guard"
    );
    assert!(
        !root
            .join("crates/codegen/grok-nix-helper/Cargo.lock")
            .is_file(),
        "grok-nix-helper must not keep a crate-local Cargo.lock"
    );
    assert!(
        !root
            .join("crates/codegen/cargo-mem-guard/Cargo.lock")
            .is_file(),
        "cargo-mem-guard must not keep a crate-local Cargo.lock"
    );
}

#[test]
fn workspace_must_not_path_vendor_audit_crates() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let manifest = read(&root, "Cargo.toml");
    for needle in [
        "third_party/bm25",
        "third_party/rhai",
        "third_party/syntect",
        "third_party/pdf_oxide",
        "third_party/ttf-parser",
        "third_party/async-openai",
        "third_party/chacha20",
    ] {
        assert!(
            !manifest.contains(needle),
            "workspace must not path-vendor {needle}:\n{manifest}"
        );
    }
    assert!(
        !root.join("third_party/bm25").exists(),
        "third_party/bm25 must not exist"
    );
    assert!(
        !root.join("third_party/rhai").exists(),
        "third_party/rhai must not exist"
    );
    assert!(
        !root.join("third_party/syntect").exists(),
        "third_party/syntect must not exist"
    );
    assert!(
        !root.join("third_party/pdf_oxide").exists(),
        "third_party/pdf_oxide must not exist"
    );
    assert!(
        !root.join("third_party/ttf-parser").exists(),
        "third_party/ttf-parser must not exist"
    );
    assert!(
        !root.join("third_party/async-openai").exists(),
        "third_party/async-openai must not exist"
    );
}

#[test]
fn workspace_lockfile_has_no_yanked_aes_chacha20_spin() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let manifest = read(&root, "Cargo.toml");
    assert!(
        manifest.contains("chacha20-v0.10.2")
            && manifest.contains("RustCrypto/stream-ciphers")
            && !manifest.contains("third_party/chacha20"),
        "workspace must patch crates.io chacha20 to git tag chacha20-v0.10.2, not a path copy:\n{manifest}"
    );
    let lock = read(&root, "Cargo.lock");
    for (name, version) in [
        ("aes", "0.9.0"),
        ("chacha20", "0.10.0"),
        ("chacha20", "0.10.1"),
        ("spin", "0.9.8"),
        ("spin", "0.10.0"),
    ] {
        let present = lock.split("[[package]]").any(|pkg| {
            pkg.lines()
                .any(|line| line.trim() == format!("name = \"{name}\""))
                && pkg
                    .lines()
                    .any(|line| line.trim() == format!("version = \"{version}\""))
        });
        assert!(!present, "Cargo.lock must not list yanked {name} {version}");
    }
    let chacha20_pkg = lock
        .split("[[package]]")
        .find(|pkg| pkg.lines().any(|line| line.trim() == "name = \"chacha20\""));
    let chacha20_pkg = chacha20_pkg.expect("Cargo.lock must list chacha20");
    assert!(
        chacha20_pkg.contains("version = \"0.10.2\""),
        "lockfile chacha20 must be 0.10.2:\n{chacha20_pkg}"
    );
    assert!(
        chacha20_pkg.contains("git+https://github.com/RustCrypto/stream-ciphers"),
        "lockfile chacha20 must be the official git tag, not crates.io yanked 0.10.0/0.10.1:\n{chacha20_pkg}"
    );
}

#[test]
fn workspace_lockfile_has_no_unmaintained_smartstring() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let manifest = read(&root, "Cargo.toml");
    assert!(
        !manifest.contains("third_party/rhai"),
        "workspace must not path-vendor rhai:\n{manifest}"
    );
}

#[test]
fn isolated_crane_package_src_does_not_load_parent_workspace() {
    let Some(root) = skip_or_root() else {
        return;
    };
    for rel in ["flake/cargo-mem-guard.nix", "flake/grok-nix-helper.nix"] {
        let src = read(&root, rel);
        assert!(
            src.contains("lib.fileset.toSource"),
            "{rel} must keep a fileset root so crane does not load the parent workspace Cargo.toml:\n{src}"
        );
        assert!(
            !src.contains("../Cargo.lock"),
            "{rel} must not vendor the monorepo Cargo.lock:\n{src}"
        );
    }
}

#[test]
fn grok_oss_package_sandbox_keeps_cargo_jobs_two() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let grok = read(&root, "flake/grok-oss.nix");
    assert!(
        grok.contains("CARGO_BUILD_JOBS = \"2\""),
        "commonArgs / grok-oss must keep CARGO_BUILD_JOBS = \"2\" for the local/GHA package sandbox"
    );
    assert!(
        grok.contains("GROK_GIT_SHA")
            || read(&root, "flake.nix")
                .contains("GROK_GIT_SHA = self.shortRev or self.dirtyShortRev or \"unknown\""),
        "grok-oss still needs GROK_GIT_SHA from shortRev/dirtyShortRev"
    );
}

#[test]
fn workspace_artifacts_drop_git_sha_quality_keeps_it() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let quality = read(&root, "flake/workspace-quality.nix");
    assert!(
        quality.contains("removeAttrs commonArgs") && quality.contains("GROK_GIT_SHA"),
        "workspaceCargoArtifacts must drop GROK_GIT_SHA via removeAttrs commonArgs"
    );
    let artifacts_block = quality
        .split("workspaceCargoArtifacts = craneLib.buildDepsOnly")
        .nth(1)
        .unwrap_or("");
    let quality_block = quality
        .split("workspace-cargo-quality = craneLib.mkCargoDerivation")
        .nth(1)
        .unwrap_or("");
    assert!(
        artifacts_block.contains("removeAttrs commonArgs"),
        "workspaceCargoArtifacts must drop GROK_GIT_SHA"
    );
    assert!(
        !quality_block.contains("removeAttrs commonArgs"),
        "workspace-cargo-quality compiles pager-bin and must keep GROK_GIT_SHA"
    );
}

#[test]
fn named_remote_cargo_source_contracts() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let named = recipe_body(&just, "remote_named_cargo");
    let test_body = recipe_body(&just, "test-remote");
    let cargo_body = recipe_body(&just, "cargo-remote");
    assert!(
        test_body.contains("remote-named-cargo"),
        "test-remote must invoke grok-nix-helper remote-named-cargo:\n{test_body}"
    );
    assert!(
        cargo_body.contains("remote-named-cargo"),
        "cargo-remote must invoke grok-nix-helper remote-named-cargo:\n{cargo_body}"
    );
    assert!(
        !named.contains("just ci") && !named.contains("just test"),
        "named remote cargo must not run host just ci/test:\n{named}"
    );
    let helper = read(
        &root,
        "crates/codegen/grok-nix-helper/src/remote_named_cargo.rs",
    );
    assert!(
        helper.contains("GROK_NIX_FORCE_REMOTE"),
        "remote-named-cargo helper must set GROK_NIX_FORCE_REMOTE"
    );
    assert!(
        helper.contains("require_remote_builder"),
        "remote-named-cargo helper must invoke require_remote_builder"
    );
    assert!(
        helper.contains("--impure"),
        "nix build must pass --impure so builtins.getEnv sees the filter"
    );
    assert!(
        helper.contains(".#workspace-cargo-named-test"),
        "helper must nix build .#workspace-cargo-named-test as one argv word"
    );
    assert!(
        helper.contains("GROK_REMOTE_TEST_ARGS"),
        "must export GROK_REMOTE_TEST_ARGS for flake getEnv"
    );
    assert!(
        helper.contains("GROK_REMOTE_CARGO_KIND"),
        "must export GROK_REMOTE_CARGO_KIND"
    );
    assert!(
        helper.contains("--no-run"),
        "helper must reject --no-run for test/nextest"
    );
    let flake = quality_src(&root);
    assert!(
        flake.contains("workspace-cargo-named-test"),
        "flake must define workspace-cargo-named-test"
    );
    assert!(
        flake.contains("cargo test --locked"),
        "named-test must run cargo test --locked"
    );
    assert!(
        flake.contains("nextest run --locked"),
        "named-test must be able to run cargo nextest run --locked"
    );
    let named_nix = read(&root, "flake/workspace-named-test.nix");
    assert!(
        named_nix.contains("--build-jobs \"$CARGO_LINK_JOBS\""),
        "named-test nextest compile/link must pass --build-jobs \"$CARGO_LINK_JOBS\":\n{named_nix}"
    );
    assert!(
        named_nix.contains("clippy-driver") && named_nix.contains("cargo build"),
        "named-test must support clippy-driver lint and cargo build kinds"
    );
    assert!(
        !named_nix.contains("--no-run"),
        "workspace-cargo-named-test must not be compile-only"
    );
    assert!(
        named_nix.contains("preferLocalBuild = false"),
        "named-test must set preferLocalBuild = false"
    );
    assert!(
        named_nix.contains("surmount-remote"),
        "named-test must require surmount-remote"
    );
    assert!(
        named_nix.contains("builtins.getEnv \"GROK_REMOTE_TEST_ARGS\""),
        "named-test must read GROK_REMOTE_TEST_ARGS via builtins.getEnv"
    );
    assert!(
        named_nix.contains("workspaceCargoArtifacts"),
        "named-test must reuse workspaceCargoArtifacts"
    );
    let clippy_kind = named_nix
        .split("clippy)")
        .nth(1)
        .and_then(|s| s.split("build)").next())
        .unwrap_or("");
    assert!(
        clippy_kind.contains("--all-targets"),
        "named-test clippy kind must cargo check --all-targets:\n{clippy_kind}"
    );
}

fn recipe_must_not_nix_build_helper(name: &str, body: &str) {
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        if t.contains("nix build") && t.contains("grok-nix-helper") {
            panic!("{name} must not nix-build grok-nix-helper:\n{line}\n{body}");
        }
        if t.contains(".#grok-nix-helper") {
            panic!("{name} must not realize .#grok-nix-helper:\n{line}\n{body}");
        }
    }
}

#[test]
fn require_remote_builder_is_justfile_preflight_without_helper() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let body = recipe_body(&just, "require_remote_builder");
    recipe_must_not_nix_build_helper("require_remote_builder", &body);
    assert!(
        !body.contains("grok_helper") && !body.contains("grok_nix_helper_bin"),
        "require_remote_builder must be justfile/uname/SSH preflight, not a grok-nix-helper trampoline:\n{body}"
    );
    assert!(
        body.contains("GROK_NIX_REMOTE_SYSTEM_FEATURES"),
        "require_remote_builder must accept an injected daemon feature list:\n{body}"
    );
    assert!(
        body.contains("ssh-keygen") && body.contains("GROK_NIX_KNOWN_HOSTS"),
        "require_remote_builder must check this account's known_hosts with ssh-keygen:\n{body}"
    );
    assert!(
        body.contains("surmount-remote"),
        "require_remote_builder must require surmount-remote:\n{body}"
    );
    assert!(
        !body.contains("StrictHostKeyChecking=no"),
        "require_remote_builder must not set StrictHostKeyChecking=no:\n{body}"
    );
    assert!(
        just.contains("check-remote:") && just.contains("require_remote_builder"),
        "check-remote must invoke require_remote_builder"
    );
    let same_path = recipe_body(&just, "test-check-remote-preflight-same-path-as-nix-ssh");
    let daemon = recipe_body(&just, "test-check-remote-preflight-remote-daemon-features");
    for (name, probe) in [
        (
            "test-check-remote-preflight-same-path-as-nix-ssh",
            same_path.as_str(),
        ),
        (
            "test-check-remote-preflight-remote-daemon-features",
            daemon.as_str(),
        ),
    ] {
        assert!(
            !probe.contains("recipe_body"),
            "{name} must not grep the require_remote_builder body; source contracts are justfile_contracts + helper .rs:\n{probe}"
        );
    }
    let helper = read(
        &root,
        "crates/codegen/grok-nix-helper/src/require_remote_builder.rs",
    );
    assert!(
        helper.contains("GROK_NIX_REMOTE_SYSTEM_FEATURES")
            || helper.contains("remote_system_features"),
        "require-remote-builder must accept an injected daemon feature list"
    );
    assert!(
        helper.contains("surmount-remote"),
        "require-remote-builder must require surmount-remote"
    );
    assert!(
        !helper.contains("StrictHostKeyChecking=no"),
        "require-remote-builder must not set StrictHostKeyChecking=no"
    );
}

#[test]
fn check_remote_and_require_remote_builder_do_not_nix_build_helper() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let remote = recipe_body(&just, "check-remote");
    let req = recipe_body(&just, "require_remote_builder");
    recipe_must_not_nix_build_helper("check-remote", &remote);
    recipe_must_not_nix_build_helper("require_remote_builder", &req);
    assert!(
        !req.contains("grok_helper") && !req.contains("grok_nix_helper_bin"),
        "require_remote_builder must not locate grok-nix-helper:\n{req}"
    );
    assert!(
        remote.contains("just require_remote_builder"),
        "check-remote must still invoke require_remote_builder:\n{remote}"
    );
}

#[test]
fn require_system_and_current_system_do_not_require_helper_binary() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let require = recipe_body(&just, "require_system");
    let current = recipe_body(&just, "current_system");
    for (name, body) in [
        ("require_system", require.as_str()),
        ("current_system", current.as_str()),
    ] {
        assert!(
            !body.contains("grok_helper")
                && !body.contains("grok_nix_helper_bin")
                && !body.contains("grok-nix-helper"),
            "{name} must not require a prebuilt grok-nix-helper (CI_SYSTEM or uname in the justfile):\n{body}"
        );
        assert!(
            !body.contains("nix-current-system.sh"),
            "{name} must not reintroduce scripts/nix-current-system.sh:\n{body}"
        );
    }
    assert!(
        current.contains("CI_SYSTEM") && current.contains("uname"),
        "current_system must map CI_SYSTEM or uname without the helper:\n{current}"
    );
    assert!(
        require.contains("just current_system")
            || (require.contains("CI_SYSTEM") && require.contains("uname")),
        "require_system must use the justfile uname/CI_SYSTEM path, not the helper:\n{require}"
    );
    assert!(
        require.contains("^[a-zA-Z0-9_]+-[a-zA-Z0-9_]+$") || require.contains("a-zA-Z0-9_"),
        "require_system must refuse unsafe CI_SYSTEM interpolation:\n{require}"
    );
    let system_line = just
        .lines()
        .find(|l| l.starts_with("system :="))
        .unwrap_or("");
    for needle in [
        "Linux-x86_64",
        "x86_64-linux",
        "Linux-aarch64|Linux-arm64",
        "aarch64-linux",
        "Darwin-x86_64",
        "x86_64-darwin",
        "Darwin-arm64",
        "aarch64-darwin",
    ] {
        assert!(
            system_line.contains(needle) && current.contains(needle),
            "parse-time system := and current_system must share uname map needle {needle}:\n{system_line}\n{current}"
        );
    }
}

#[test]
fn grok_helper_does_not_exec_empty_helper_path() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let helper = recipe_body(&just, "grok_helper");
    assert!(
        !helper.contains("exec \"$(just grok_nix_helper_bin)\""),
        "grok_helper must not exec a command substitution of grok_nix_helper_bin (empty path becomes exec: : not found):\n{helper}"
    );
    assert!(
        helper.contains("helper=") && helper.contains("grok_nix_helper_bin"),
        "grok_helper must assign the path from grok_nix_helper_bin before exec:\n{helper}"
    );
    assert!(
        helper.contains("-z") || helper.contains("empty"),
        "grok_helper must refuse an empty helper path:\n{helper}"
    );
    assert!(
        helper.contains("grok-nix-helper"),
        "grok_helper must fail loud with the helper name, not exec: : not found:\n{helper}"
    );
    assert!(
        helper.contains("exec \"${helper}\"") || helper.contains("exec \"$helper\""),
        "grok_helper must exec the assigned helper path:\n{helper}"
    );
    assert!(
        !just.contains("exec \"$(just grok_nix_helper_bin)\""),
        "no recipe may exec grok_nix_helper_bin via command substitution (empty path is exec: : not found)"
    );
}

#[test]
fn grok_nix_helper_bin_locate_order_does_not_cargo_on_force_remote() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let bin = recipe_body(&just, "grok_nix_helper_bin");
    assert!(
        bin.contains("GROK_NIX_HELPER") && bin.contains("command -v grok-nix-helper"),
        "grok_nix_helper_bin must locate GROK_NIX_HELPER then PATH:\n{bin}"
    );
    assert!(
        bin.contains("result/bin/grok-nix-helper")
            && bin.contains("target/release/grok-nix-helper")
            && bin.contains("target/debug/grok-nix-helper"),
        "grok_nix_helper_bin locate order is GROK_NIX_HELPER, PATH, result/bin, crate target:\n{bin}"
    );
    assert!(
        !bin.contains("cargo build") && !bin.contains("cargo run") && !bin.contains("cargo test"),
        "grok_nix_helper_bin must not cargo/rustc the helper on this laptop:\n{bin}"
    );
    recipe_must_not_nix_build_helper("grok_nix_helper_bin", &bin);
    assert!(
        !bin.contains("GROK_NIX_FORCE_REMOTE"),
        "GROK_NIX_FORCE_REMOTE must not nix-build the helper; locate GROK_NIX_HELPER, PATH, result/bin, crate target only:\n{bin}"
    );
    assert!(
        !bin.contains("nix shell .#grok-nix-helper"),
        "do not tell the operator to nix shell / realize the helper first:\n{bin}"
    );
    for line in bin.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        assert!(
            !t.contains("nix build"),
            "grok_nix_helper_bin must not nix-build (later recipes locate only):\n{line}"
        );
    }
}

#[test]
fn check_remote_exports_force_remote_before_require_remote_builder() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let remote = recipe_body(&just, "check-remote");
    let force_pos = remote.find("GROK_NIX_FORCE_REMOTE=1").unwrap_or(usize::MAX);
    let req_pos = remote.find("require_remote_builder").unwrap_or(usize::MAX);
    assert!(
        force_pos < req_pos,
        "check-remote must export GROK_NIX_FORCE_REMOTE=1 before require_remote_builder so later nix_retry is force-remote (preflight itself does not nix-build the helper):\n{remote}"
    );
    assert!(
        remote.contains("just require_system") && remote.contains("just require_remote_builder"),
        "check-remote must still invoke require_system and require_remote_builder:\n{remote}"
    );
    for name in ["cargo-remote", "test-remote"] {
        let body = recipe_body(&just, name);
        assert!(
            body.contains("GROK_NIX_FORCE_REMOTE=1"),
            "{name} already force-remotes; export GROK_NIX_FORCE_REMOTE=1 before locating the helper:\n{body}"
        );
        assert!(
            !body.contains("exec \"$(just grok_nix_helper_bin)\""),
            "{name} must not exec grok_nix_helper_bin via command substitution:\n{body}"
        );
    }
}

#[test]
fn just_update_refreshes_workspace_and_flake_locks() {
    let Some(root) = skip_or_root() else {
        return;
    };
    let just = read(&root, "justfile");
    let update = recipe_body(&just, "update");
    let ws = must_contain_at(
        &update,
        "cargo update --manifest-path Cargo.toml",
        "just update workspace Cargo.lock",
    );
    let flake = must_contain_at(&update, "nix flake update", "just update flake.lock");
    assert!(
        !update.contains("crates/codegen/cargo-mem-guard/Cargo.toml")
            && !update.contains("crates/codegen/grok-nix-helper/Cargo.toml"),
        "just update must refresh one workspace lock, not crate-local locks:\n{update}"
    );
    assert!(
        ws < flake,
        "just update order is workspace Cargo.lock, then flake.lock:\n{update}"
    );
    let code = update
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("check-remote")
            && !code.contains("cargo build")
            && !code.contains("cargo test")
            && !code.contains("cargo check")
            && !code.contains("rustc"),
        "just update must not compile and must not run just check-remote:\n{update}"
    );
}
