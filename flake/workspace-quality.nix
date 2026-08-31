# Preserved flake outputs: workspace-cargo-quality (packages; not in checks).
# Also exports workspaceCargoArtifacts and workspaceCargoJobsFromCores for
# flake/workspace-named-test.nix.
{
  pkgs,
  lib,
  craneLib,
  commonArgs,
  nativeBuildInputs,
  buildInputs,
}:
let
  # Full workspace cargo gate matching `just test` / `just check`:
  # fmt, then clippy -D warnings (--workspace --all-targets; members
  # include cargo-mem-guard and grok-nix-helper), then cargo nextest
  # run (tests actually execute), then doctests. Workspace nextest
  # covers those member crates; do not add a late cargo test
  # --manifest-path after nextest.
  # `just check-remote` realizes this on the Nix remote builder.
  # Not in `checks` so `nix flake check` / GHA stay local and do not
  # need the builder.
  # preferLocalBuild=false is not enough: Nix still uses a local
  # nixbld worker when this machine has the required features.
  # --option system-features is also not enough: this host's daemon
  # still advertises big-parallel (Nix default for a many-core box;
  # see https://nix.dev/manual/nix/2.28/command-ref/conf-file.html#conf-system-features
  # accessed: 2026-08-18). Pin rustc to surmount-remote, a machines-file
  # feature this laptop never auto-detects. The ssh-ng builder must
  # advertise it. Tiny crane vendor unpacks may stay untagged
  # (preferLocalBuild). Force-remote still passes max-jobs 0 on the
  # caller, so Nix 2.4+ schedules those FODs on the remote instead
  # of curling crates.io or static.rust-lang.org on this laptop
  # (see https://github.com/NixOS/nix/issues/5646 accessed: 2026-08-23).
  # Override CARGO_BUILD_JOBS=2 from commonArgs (that cap is for the
  # low-memory package sandbox). Advertise 64 Nix cores on the builder
  # and keep cargo at 32 so one workspace clippy is not 8-wide and is
  # less likely to OOM than 64 rustc processes at once.
  # nextest compile links every test binary. 32 parallel mold links
  # were SIGKILL'd (ld returned 137; 128+9) under the builder
  # nix-daemon 32GiB MemoryMax. Host MemAvailable is larger;
  # cargo-mem-guard reads /proc/meminfo and would not restart.
  # Clippy/check stay at CARGO_BUILD_JOBS. CARGO_LINK_JOBS caps
  # nextest --build-jobs (and cargo test --doc --jobs) at 4. Do not
  # drop --locked. Do not skip tests.
  # Crane defaults CARGO_PROFILE to release. cargo check and clippy
  # skip LLVM, so codegen-units does not fan out a Checking rustc;
  # --release still type-checks at opt-level 3 on one thread per
  # crate. Use the same dev profile as local `just test-clippy`.
  # Pass cargo --jobs on argv from NIX_BUILD_CORES (nix --cores),
  # n = min(NIX_BUILD_CORES, 32). Do not floor from CARGO_BUILD_JOBS:
  # crane preBuild can copy NIX_BUILD_CORES=1 into that env and then
  # a 1-core assignment stays one rustc. If cores is 0 or 1, use 32
  # so the remote quality gate is not a single clippy-driver. cargo
  # fmt rejects --jobs; check/build/test get it. cargo nextest
  # run uses CARGO_LINK_JOBS for --build-jobs (compile and link);
  # nextest -j is test processes (CARGO_BUILD_JOBS). cargo 1.97.1
  # has no global `cargo --jobs N` (tip:
  # `check --jobs`). Put --jobs after the subcommand: `cargo check
  # --jobs N`. Do not run `cargo clippy`: that is an external
  # cargo-clippy binary. The outer cargo may start a 1-token GNU
  # jobserver from available_parallelism() (often 1 in a Nix
  # sandbox); inner `--jobs N` is then ignored and you get one
  # clippy-driver. Workspace lint is builtin `cargo check` with
  # RUSTC_WORKSPACE_WRAPPER=clippy-driver under a GNU make
  # jobserver with $CARGO_BUILD_JOBS tokens. Drop Nix MAKEFLAGS
  # first (may be 1 token), then make -j$CARGO_BUILD_JOBS.
  # Never raise Nix max-jobs for this.
  workspaceCargoJobsFromCores = ''
    cargoJobs="''${NIX_BUILD_CORES:-32}"
    case "$cargoJobs" in
      "" | *[!0-9]*) cargoJobs=32 ;;
    esac
    if [ "$cargoJobs" -gt 32 ]; then
      cargoJobs=32
    fi
    if [ "$cargoJobs" -lt 2 ]; then
      cargoJobs=32
    fi
    export CARGO_BUILD_JOBS="$cargoJobs"
    # Six concurrent mold links of workspace test binaries were
    # SIGKILL'd in one quality run (ld returned 137). Cap compile
    # and link below clippy's 32 jobs. Host RAM is not the
    # nix-daemon memory cgroup.
    linkJobs="$CARGO_BUILD_JOBS"
    if [ "$linkJobs" -gt 4 ]; then
      linkJobs=4
    fi
    export CARGO_LINK_JOBS="$linkJobs"
    unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
    echo "workspace cargo jobs=$CARGO_BUILD_JOBS link jobs=$CARGO_LINK_JOBS NIX_BUILD_CORES=''${NIX_BUILD_CORES:-unset}"
    workspace_run_make_jobserver() {
      unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
      if [ "$#" -lt 1 ]; then
        echo "workspace_run_make_jobserver: missing command" >&2
        exit 2
      fi
      mk="''${TMPDIR:-/tmp}/workspace-cargo-jobserver.mk"
      {
        printf 'all:\n\t+'
        printf '%q ' "$@"
        printf '\n'
      } > "$mk"
      echo "workspace cargo make -j$CARGO_BUILD_JOBS jobserver: $*"
      make -j"$CARGO_BUILD_JOBS" --no-print-directory -f "$mk"
    }
  '';
  # Dummy deps stubs do not need the pager build-id. GROK_GIT_SHA from
  # dirtyShortRev would bust this drv on any dirty tree, even files
  # cargo filter drops. grok-oss and quality keep it (build.rs).
  workspaceCargoArtifacts = craneLib.buildDepsOnly (
    (lib.removeAttrs commonArgs [ "GROK_GIT_SHA" ])
    // {
      pname = "workspace-cargo-quality";
      preferLocalBuild = false;
      requiredSystemFeatures = [
        "big-parallel"
        "surmount-remote"
      ];
      # Dev-profile CFLAGS are -O0. Nix gcc wrapping still injects
      # _FORTIFY_SOURCE; jemalloc configure then compiles probes with
      # -O0 -Werror and dies with "cannot determine return type of
      # strerror_r". Same reason ciLowMemEnv drops fortify for host
      # cargo. fortify3 is the nixos-unstable sibling of fortify.
      hardeningDisable = [
        "fortify"
        "fortify3"
      ];
      enableParallelBuilding = true;
      CARGO_BUILD_JOBS = "32";
      CARGO_PROFILE = "dev";
      buildPhaseCargoCommand = ''
        ${workspaceCargoJobsFromCores}
        workspace_run_make_jobserver cargo check --profile "$CARGO_PROFILE" --jobs "$CARGO_BUILD_JOBS" --locked --all-targets
        workspace_run_make_jobserver cargo build --profile "$CARGO_PROFILE" --jobs "$CARGO_BUILD_JOBS" --locked
      '';
    }
  );

  workspace-cargo-quality = craneLib.mkCargoDerivation (
    commonArgs
    // {
      pname = "workspace-cargo-quality";
      cargoArtifacts = workspaceCargoArtifacts;
      pnameSuffix = "";
      preferLocalBuild = false;
      requiredSystemFeatures = [
        "big-parallel"
        "surmount-remote"
      ];
      hardeningDisable = [
        "fortify"
        "fortify3"
      ];
      enableParallelBuilding = true;
      CARGO_BUILD_JOBS = "32";
      CARGO_PROFILE = "dev";
      # Same cargo steps as `just test`. nextest is not in commonArgs.
      # ripgrep is required: the receipt greps nextest.log with `rg -a -F
      # 'Summary ['`. Without it, a green nextest run still exits 2.
      nativeBuildInputs = nativeBuildInputs ++ [
        pkgs.cargo-nextest
        pkgs.git
        pkgs.python3
        pkgs.ripgrep
      ];
      # One workspace Cargo.lock (cargo-mem-guard and grok-nix-helper
      # are members). Do not vendor leftover crate-local locks.
      # Gate only: skip post-clippy zstd of target (not an artifacts cache).
      doInstallCargoArtifacts = false;
      # Order: fmt, then workspace clippy --all-targets, then nextest, doctests.
      doCheck = false;
      buildPhaseCargoCommand = ''
        ${workspaceCargoJobsFromCores}
        export LD_LIBRARY_PATH="${lib.makeLibraryPath buildInputs}''${LD_LIBRARY_PATH:+:''${LD_LIBRARY_PATH}}"
        export RULES_RUST_RUNFILES_WORKSPACE_NAME="''${RULES_RUST_RUNFILES_WORKSPACE_NAME:-grok-oss}"
        export GROK_DISABLE_SHARED_HARNESS_SECRETS="''${GROK_DISABLE_SHARED_HARNESS_SECRETS:-1}"
        export GROK_CREDENTIALS_FORCE_FILE="''${GROK_CREDENTIALS_FORCE_FILE:-1}"
        export GROK_TRUST_LOOPBACK_CLI_CHAT_PROXY="''${GROK_TRUST_LOOPBACK_CLI_CHAT_PROXY:-1}"
        unset NO_COLOR
        unset CARGO_TERM_COLOR
        unset OPENROUTER_API_KEY
        cargo fmt --all -- --check
        clippyDriver="$(command -v clippy-driver)"
        if [ -z "$clippyDriver" ] || [ ! -x "$clippyDriver" ]; then
          echo "workspace-cargo-quality: clippy-driver not on PATH" >&2
          exit 2
        fi
        export RUSTC_WORKSPACE_WRAPPER="$clippyDriver"
        export CLIPPY_ARGS="-D__CLIPPY_HACKERY__warnings__CLIPPY_HACKERY__"
        workspace_run_make_jobserver cargo check --profile "$CARGO_PROFILE" --jobs "$CARGO_BUILD_JOBS" --workspace --all-targets --locked
        unset RUSTC_WORKSPACE_WRAPPER CLIPPY_ARGS
        unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
        nextest_log="$NIX_BUILD_TOP/nextest.log"
        set -o pipefail
        if ! command -v rg >/dev/null 2>&1; then
          echo "workspace-cargo-quality: rg not on PATH" >&2
          exit 2
        fi
        # nextest writes the final Summary line on stderr. stdout-only tee
        # leaves nextest.log without that line and the receipt exits 2.
        CARGO_BUILD_JOBS="$CARGO_LINK_JOBS" cargo nextest run --workspace --locked --build-jobs "$CARGO_LINK_JOBS" -j "$CARGO_BUILD_JOBS" 2>&1 | tee "$nextest_log"
        unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
        workspace_run_make_jobserver cargo test --workspace --doc --locked --profile "$CARGO_PROFILE" --jobs "$CARGO_LINK_JOBS"
        summary="$(rg -a -F -n 'Summary [' "$nextest_log" | tail -n 1 || true)"
        if [ -z "$summary" ]; then
          echo "workspace-cargo-quality: nextest Summary line missing" >&2
          exit 2
        fi
        {
          echo "workspace-cargo-quality receipt"
          echo "fmt: cargo fmt --all -- --check"
          echo "clippy: cargo check --workspace --all-targets (clippy-driver -D warnings)"
          echo "$summary"
          echo "doctests: cargo test --workspace --doc"
        } > "$NIX_BUILD_TOP/quality-summary.txt"
      '';
      installPhase = ''
        runHook preInstall
        mkdir -p "$out"
        if [ ! -f "$NIX_BUILD_TOP/quality-summary.txt" ]; then
          echo "workspace-cargo-quality: missing quality-summary.txt" >&2
          exit 2
        fi
        cp "$NIX_BUILD_TOP/quality-summary.txt" "$out/quality-summary.txt"
        runHook postInstall
      '';
    }
  );
in
{
  inherit
    workspaceCargoJobsFromCores
    workspaceCargoArtifacts
    workspace-cargo-quality
    ;
}
