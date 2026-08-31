# Preserved flake outputs: workspace-cargo-named-test (packages; not in checks).
{
  pkgs,
  lib,
  craneLib,
  commonArgs,
  nativeBuildInputs,
  buildInputs,
  workspaceCargoArtifacts,
  workspaceCargoJobsFromCores,
}:
let
  # Named cargo test / clippy / build / check on the same remote
  # builder as workspace-cargo-quality (surmount-remote, not this
  # laptop). One cargo kind plus a filter, not the full quality
  # chain (fmt, then workspace clippy --all-targets, then
  # nextest and doctests). `just test-remote` and `just
  # cargo-remote` set GROK_REMOTE_CARGO_KIND and GROK_REMOTE_TEST_ARGS
  # (base64 of NUL-separated argv) and `nix build --impure` this attr.
  # Tests actually run (cargo test / cargo nextest run). Not in
  # `checks`. GitHub Actions must not realize this. Pure eval
  # (empty env) still produces a drv; the build fails until the
  # just recipe supplies a kind and a filter.
  workspace-cargo-named-test = craneLib.mkCargoDerivation (
    commonArgs
    // {
      pname = "workspace-cargo-named-test";
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
      nativeBuildInputs = nativeBuildInputs ++ [
        pkgs.cargo-nextest
        pkgs.git
        pkgs.python3
      ];
      doInstallCargoArtifacts = false;
      doCheck = false;
      GROK_REMOTE_TEST_ARGS = builtins.getEnv "GROK_REMOTE_TEST_ARGS";
      GROK_REMOTE_CARGO_KIND = builtins.getEnv "GROK_REMOTE_CARGO_KIND";
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
        kind="''${GROK_REMOTE_CARGO_KIND:-}"
        args_b64="''${GROK_REMOTE_TEST_ARGS:-}"
        if [ -z "$kind" ] || [ -z "$args_b64" ]; then
          echo "workspace-cargo-named-test: just test-remote / just cargo-remote must pass a cargo kind and a filter through nix build --impure." >&2
          exit 2
        fi
        case "$kind" in
          test|nextest|clippy|build|check) ;;
          *)
            echo "workspace-cargo-named-test: GROK_REMOTE_CARGO_KIND must be test, nextest, clippy, build, or check." >&2
            exit 2
            ;;
        esac
        mapfile -d $'\0' -t remote_args < <(printf '%s' "$args_b64" | base64 -d)
        if [ "''${#remote_args[@]}" -lt 1 ]; then
          echo "workspace-cargo-named-test: filter argv is empty." >&2
          exit 2
        fi
        unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
        case "$kind" in
          test)
            workspace_run_make_jobserver cargo test --locked --profile "$CARGO_PROFILE" --jobs "$CARGO_LINK_JOBS" "''${remote_args[@]}"
            ;;
          nextest)
            CARGO_BUILD_JOBS="$CARGO_LINK_JOBS" cargo nextest run --locked --build-jobs "$CARGO_LINK_JOBS" -j "$CARGO_BUILD_JOBS" "''${remote_args[@]}"
            ;;
          clippy)
            clippyDriver="$(command -v clippy-driver)"
            if [ -z "$clippyDriver" ] || [ ! -x "$clippyDriver" ]; then
              echo "workspace-cargo-named-test: clippy-driver not on PATH" >&2
              exit 2
            fi
            export RUSTC_WORKSPACE_WRAPPER="$clippyDriver"
            export CLIPPY_ARGS="-D__CLIPPY_HACKERY__warnings__CLIPPY_HACKERY__"
            workspace_run_make_jobserver cargo check --profile "$CARGO_PROFILE" --jobs "$CARGO_BUILD_JOBS" --locked --all-targets "''${remote_args[@]}"
            unset RUSTC_WORKSPACE_WRAPPER CLIPPY_ARGS
            ;;
          build)
            workspace_run_make_jobserver cargo build --profile "$CARGO_PROFILE" --jobs "$CARGO_BUILD_JOBS" --locked "''${remote_args[@]}"
            ;;
          check)
            workspace_run_make_jobserver cargo check --profile "$CARGO_PROFILE" --jobs "$CARGO_BUILD_JOBS" --locked "''${remote_args[@]}"
            ;;
        esac
      '';
    }
  );
in
{
  inherit workspace-cargo-named-test;
}
