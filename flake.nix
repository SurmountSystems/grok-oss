{
  description = "Grok OSS - unofficial open-source fork of xAI Grok Build";

  # Input surface is intentionally small (no flake-utils / systems).
  # github: still uses the tarball API, but with fewer inputs and NIX_CONFIG
  # download-attempts + just nix_retry we survive free-GHA 502/503s.
  # Avoid git+https for nixpkgs: a full clone is multi-GB and more fragile
  # on free runners than a single tarball of the locked rev.
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      crane,
    }:
    let
      # Same default set flake-utils used; no extra flake input to fetch.
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;

      perSystem = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (pkgs) lib;

          # Match rust-toolchain.toml (channel 1.97.1 + clippy/rustfmt).
          # FOD SRI for channel-rust-1.97.1.toml: when rust-lang rewrites the
          # manifest, just check fails with hash mismatch — set sha256 to the
          # "got:" value from the error.
          rustToolchain = fenix.packages.${system}.fromToolchainFile {
            file = ./rust-toolchain.toml;
            sha256 = "sha256-A1abGIbOtcBSdrUMhDGrER3pRM1hQP4fp9gh3Y4PKc8=";
          };

          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          # ------------------------------------------------------------------
          # cargo-mem-guard
          #
          # Standalone crate under crates/codegen/ (workspace-excluded). Built
          # with a fileset root so crane never sees the monorepo Cargo.toml.
          # On Linux the binary is wrapped with mold on PATH and mold-friendly
          # defaults -- pure Nix, no host PATH / bash scripts.
          # ------------------------------------------------------------------
          memGuardRoot = ./crates/codegen/cargo-mem-guard;

          memGuardSrc = lib.fileset.toSource {
            root = memGuardRoot;
            fileset = lib.fileset.unions [
              (memGuardRoot + /Cargo.toml)
              (memGuardRoot + /Cargo.lock)
              (memGuardRoot + /src)
            ];
          };

          memGuardCrate = craneLib.crateNameFromCargoToml {
            cargoToml = memGuardRoot + /Cargo.toml;
          };

          memGuardCommonArgs = {
            inherit (memGuardCrate) pname version;
            src = memGuardSrc;
            strictDeps = true;
            # Pure std; no openssl / dbus / protoc.
            meta = {
              description = "Memory-aware cargo wrapper for constrained CI runners";
              homepage = "https://github.com/SurmountSystems/grok-oss";
              license = lib.licenses.asl20;
              mainProgram = "cargo-mem-guard";
              platforms = lib.platforms.unix;
            };
          };

          # Install package only (no unit tests here). Tests live solely in
          # checks.cargo-mem-guard-tests so free GHA / mem-guard does not pay
          # for the suite twice (package doCheck + separate check attr).
          cargo-mem-guard-unwrapped = craneLib.buildPackage (
            memGuardCommonArgs
            // {
              doCheck = false;
            }
          );

          # Unit tests as the single flake check for this crate.
          cargo-mem-guard-tests = craneLib.cargoTest (
            memGuardCommonArgs
            // {
              cargoArtifacts = craneLib.buildDepsOnly memGuardCommonArgs;
            }
          );

          # Bake mold into the runtime closure on Linux so CARGO_MEM_USE_MOLD
          # works without relying on the ambient host PATH.
          cargo-mem-guard =
            if pkgs.stdenv.isLinux then
              pkgs.symlinkJoin {
                name = "${memGuardCrate.pname}-${memGuardCrate.version}";
                paths = [ cargo-mem-guard-unwrapped ];
                nativeBuildInputs = [ pkgs.makeWrapper ];
                postBuild = ''
                  wrapProgram $out/bin/cargo-mem-guard \
                    --prefix PATH : ${lib.makeBinPath [ pkgs.mold ]} \
                    --set-default CARGO_MEM_USE_MOLD 1
                '';
                meta = cargo-mem-guard-unwrapped.meta // {
                  description = "${cargo-mem-guard-unwrapped.meta.description} (with mold)";
                };
              }
            else
              cargo-mem-guard-unwrapped;

          # ------------------------------------------------------------------
          # grok-oss monorepo (crane)
          # ------------------------------------------------------------------
          src = lib.cleanSourceWith {
            src = ./.;
            filter =
              path: type:
              let
                base = baseNameOf path;
              in
              (craneLib.filterCargoSources path type)
              || lib.hasInfix "/crates/" path
              || lib.hasInfix "/prod/" path
              || lib.hasInfix "/third_party/" path
              || lib.hasInfix "/bin/" path
              || lib.hasInfix "/.config/" path
              || base == "rust-toolchain.toml"
              || base == "clippy.toml"
              || base == "rustfmt.toml"
              || base == "nextest.toml"
              || base == "protoc";
          };

          nativeBuildInputs =
            with pkgs;
            [
              pkg-config
              protobuf
              cmake
              perl
              ripgrep
              makeWrapper
            ]
            ++ lib.optionals stdenv.isLinux [
              # Faster, leaner final links on Linux (helps free GHA RAM peaks).
              mold
            ];

          buildInputs =
            with pkgs;
            [ openssl ]
            ++ lib.optionals stdenv.isLinux [ dbus ]
            ++ lib.optionals stdenv.isDarwin [
              darwin.apple_sdk.frameworks.Security
              darwin.apple_sdk.frameworks.SystemConfiguration
            ];

          # Linux: prefer mold for links inside pure crane builds.
          moldRustflags = lib.optionalString pkgs.stdenv.isLinux "-C link-arg=-fuse-ld=mold";

          commonArgs = {
            inherit src nativeBuildInputs buildInputs;
            strictDeps = true;
            pname = "grok-oss";
            version =
              (craneLib.crateNameFromCargoToml {
                cargoToml = ./crates/codegen/xai-grok-pager-bin/Cargo.toml;
              }).version;
            PROTOC = "${pkgs.protobuf}/bin/protoc";
            OPENSSL_NO_VENDOR = "1";
            GROK_TOOLS_BUNDLE_RG_PATH = "${pkgs.ripgrep}/bin/rg";
            GROK_SHELL_BUNDLE_RG_PATH = "${pkgs.ripgrep}/bin/rg";
            GROK_GIT_SHA = self.shortRev or self.dirtyShortRev or "unknown";
            # Cap cargo fan-out inside the pure sandbox (free GHA ~16GB).
            CARGO_BUILD_JOBS = "2";
            RUSTFLAGS = moldRustflags;
          };

          cargoArtifacts = craneLib.buildDepsOnly (
            commonArgs
            // {
              cargoExtraArgs = "-p xai-grok-pager-bin";
            }
          );

          grok-oss = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoExtraArgs = "-p xai-grok-pager-bin";
              postInstall = ''
                wrapProgram $out/bin/grok-oss \
                  --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath buildInputs}
              '';
              meta = {
                description = "Unofficial open-source Grok Build coding agent (Surmount fork)";
                homepage = "https://github.com/SurmountSystems/grok-oss";
                license = lib.licenses.asl20;
                mainProgram = "grok-oss";
              };
            }
          );

          cargoCheck = craneLib.mkCargoDerivation (
            commonArgs
            // {
              inherit cargoArtifacts;
              pnameSuffix = "-check";
              buildPhaseCargoCommand = "cargoWithProfile check -p xai-grok-pager-bin --locked";
            }
          );

          # Full workspace cargo gate matching `just test` / `just check`:
          # fmt, clippy -D warnings (--all-targets), cargo nextest run (tests
          # actually execute), doctests, and workspace-excluded cargo-mem-guard
          # tests. `just check-remote` realizes this on the Nix remote builder.
          # Not in `checks` so `nix flake check` / GHA stay local and do not
          # need the builder.
          # preferLocalBuild=false is not enough: Nix still uses a local
          # nixbld worker when this machine has the required features.
          # --option system-features is also not enough: this host's daemon
          # still advertises big-parallel (Nix default for a many-core box;
          # see https://nix.dev/manual/nix/2.28/command-ref/conf-file.html#conf-system-features
          # accessed: 2026-08-18). Pin rustc to surmount-remote, a machines-file
          # feature this laptop never auto-detects. The ssh-ng builder must
          # advertise it. Tiny crane vendor unpacks stay untagged so they may
          # run here (do not pair them with max-jobs=0).
          # Override CARGO_BUILD_JOBS=2 from commonArgs (that cap is for the
          # low-memory package sandbox). Advertise 64 Nix cores on the builder
          # and keep cargo at 32 so one workspace clippy is not 8-wide and is
          # less likely to OOM than 64 rustc processes at once.
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
          # run uses CARGO_BUILD_JOBS for rustc; nextest --jobs is test
          # processes. cargo 1.97.1 has no global `cargo --jobs N` (tip:
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
            unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
            echo "workspace cargo jobs=$CARGO_BUILD_JOBS NIX_BUILD_CORES=''${NIX_BUILD_CORES:-unset}"
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
              nativeBuildInputs = nativeBuildInputs ++ [
                pkgs.cargo-nextest
                pkgs.git
                pkgs.python3
              ];
              cargoVendorDir = craneLib.vendorMultipleCargoDeps {
                cargoLockList = [
                  ./Cargo.lock
                  ./crates/codegen/cargo-mem-guard/Cargo.lock
                ];
              };
              # Gate only: skip post-clippy zstd of target (not an artifacts cache).
              doInstallCargoArtifacts = false;
              # Tests run in buildPhaseCargoCommand (same order as `just test`).
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
                cargo nextest run --workspace --locked
                unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
                workspace_run_make_jobserver cargo test --workspace --doc --locked --profile "$CARGO_PROFILE" --jobs "$CARGO_BUILD_JOBS"
                unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
                workspace_run_make_jobserver cargo test --manifest-path crates/codegen/cargo-mem-guard/Cargo.toml --locked --profile "$CARGO_PROFILE" --jobs "$CARGO_BUILD_JOBS"
              '';
            }
          );

          openrouter-credentials = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoExtraArgs = "-p xai-grok-shell";
              cargoTestExtraArgs = "--test openrouter_credentials";
              preCheck = ''
                export LD_LIBRARY_PATH="${lib.makeLibraryPath buildInputs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
              '';
            }
          );

          # Named cargo test / clippy / build / check on the same remote
          # builder as workspace-cargo-quality (surmount-remote, not this
          # laptop). `just test-remote` and `just cargo-remote` set
          # GROK_REMOTE_CARGO_KIND and GROK_REMOTE_TEST_ARGS (base64 of
          # NUL-separated argv) and `nix build --impure` this attr.
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
                    workspace_run_make_jobserver cargo test --locked --profile "$CARGO_PROFILE" --jobs "$CARGO_BUILD_JOBS" "''${remote_args[@]}"
                    ;;
                  nextest)
                    cargo nextest run --locked "''${remote_args[@]}"
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

          # ------------------------------------------------------------------
          # Host CI toolchain (free GHA / low-mem)
          #
          # A single buildEnv so consumers can:
          #   nix shell .#ci-tools -c cargo-mem-guard -- cargo check ...
          #   nix develop .#ci
          # without assembling PATH by hand or writing bash wrappers.
          # ------------------------------------------------------------------
          ciLowMemEnv = {
            CARGO_MEM_JOBS_START = "2";
            CARGO_MEM_JOBS_MIN = "1";
            CARGO_MEM_HIGH_WATER = "0.15";
            CARGO_MEM_MAX_RESTARTS = "3";
            CARGO_MEM_USE_MOLD = if pkgs.stdenv.isLinux then "1" else "0";
            PROTOC = "${pkgs.protobuf}/bin/protoc";
            OPENSSL_NO_VENDOR = "1";
            GROK_TOOLS_BUNDLE_RG_PATH = "${pkgs.ripgrep}/bin/rg";
            GROK_SHELL_BUNDLE_RG_PATH = "${pkgs.ripgrep}/bin/rg";
            PKG_CONFIG_PATH = lib.makeSearchPathOutput "dev" "lib/pkgconfig" (
              [ pkgs.openssl ] ++ lib.optionals pkgs.stdenv.isLinux [ pkgs.dbus ]
            );
            LD_LIBRARY_PATH = lib.makeLibraryPath (
              [ pkgs.openssl ] ++ lib.optionals pkgs.stdenv.isLinux [ pkgs.dbus ]
            );
            # mkShell injects NIX_HARDENING_ENABLE with fortify. jemalloc's
            # configure runs C probes under -O0 -Werror; fortify then emits
            # "_FORTIFY_SOURCE requires -O" and the probe fails as
            # "cannot determine return type of strerror_r". Host cargo CI
            # is not a pure nix build -- disable fortify for configure probes.
            NIX_HARDENING_ENABLE = "bindnow format pic relro stackprotector strictoverflow";
          };

          # Tiny bootstrap package: locked nixpkgs `just` only (no rustc).
          # GHA cold-start uses `nix shell .#just -c just ci` so free runners
          # never hit unpinned `nix shell nixpkgs#just` registry tarballs.
          # Note: evaluating .#just still loads full flake inputs (nixpkgs /
          # fenix / crane) once; only the realized closure is just-only.
          justPkg = pkgs.just;

          # Host-CI toolchain only: rustc, nextest, mem-guard, build deps, git,
          # python3. Do NOT add desktop audio recorders (pw-record/parec/arecord)
          # — quality tests must not see mic tools just because the developer
          # desktop has them.
          # `just cargo-ci` under CI_LOW_MEM scrubs PATH to /nix/store allowlist
          # (see scripts/with-ci-hermetic-path.sh); git + python3 must live here
          # so scrub does not drop VCS (cargo git deps / git unit tests) or the
          # interpreter (cgroup_memory_test + mock LSP e2e spawn `python3`).
          ci-tools = pkgs.buildEnv {
            name = "grok-oss-ci-tools";
            paths =
              [
                rustToolchain
                cargo-mem-guard
                # Process-per-test runner used by `just test-unit`.
                pkgs.cargo-nextest
                pkgs.pkg-config
                pkgs.protobuf
                pkgs.cmake
                pkgs.openssl
                pkgs.perl
                pkgs.ripgrep
                pkgs.git
                # Store `python3` so hermetic PATH scrub still resolves tests that
                # spawn it (cgroup memory mocks, mock LSP e2e). Slim interpreter
                # only — not in flake checks graphs as a separate heavy attr.
                pkgs.python3
                justPkg
              ]
              ++ lib.optionals pkgs.stdenv.isLinux [
                pkgs.mold
                pkgs.dbus
              ];
            pathsToLink = [
              "/bin"
              "/lib"
              "/include"
              "/lib/pkgconfig"
              "/share"
            ];
            meta = {
              description = "Host CI toolchain: fenix rustc, cargo-nextest, cargo-mem-guard, mold, git, python3, build deps";
              homepage = "https://github.com/SurmountSystems/grok-oss";
              license = lib.licenses.asl20;
            };
          };

          devShell = pkgs.mkShell {
            packages = [
              rustToolchain
              pkgs.rust-analyzer
              pkgs.pkg-config
              pkgs.protobuf
              pkgs.cmake
              pkgs.openssl
              pkgs.git
              pkgs.ripgrep
              cargo-mem-guard
              pkgs.cargo-nextest
            ]
            ++ lib.optionals pkgs.stdenv.isLinux [
              pkgs.dbus
              pkgs.mold
            ];

            # Share host-cargo env with .#ci so jemalloc configure works here too
            # (fortify-off via NIX_HARDENING_ENABLE; see ciLowMemEnv comment).
            inherit (ciLowMemEnv)
              PROTOC
              OPENSSL_NO_VENDOR
              GROK_TOOLS_BUNDLE_RG_PATH
              GROK_SHELL_BUNDLE_RG_PATH
              NIX_HARDENING_ENABLE
              ;

            shellHook = ''
              echo "Grok OSS dev shell (fenix from rust-toolchain.toml)"
              echo "  cargo build -p xai-grok-pager-bin --release"
              echo "  nix run .#cargo-mem-guard -- cargo check -p xai-grok-pager-bin --locked"
              echo "  nix build .#grok-oss"
              echo "  nix build .#cargo-mem-guard"
              echo "  nix shell .#ci-tools"
            '';
          };

          # Free-GHA / low-mem host builds: same tools as packages.ci-tools,
          # plus the pressure-restart defaults as shell env.
          ciShell = pkgs.mkShell {
            packages = [ ci-tools ];
            env = ciLowMemEnv;
          };

        in
        {
          inherit
            grok-oss
            cargo-mem-guard
            cargo-mem-guard-unwrapped
            cargo-mem-guard-tests
            cargoCheck
            workspace-cargo-quality
            workspace-cargo-named-test
            openrouter-credentials
            justPkg
            ci-tools
            devShell
            ciShell
            ;
        }
      );
    in
    {
      packages = forAllSystems (
        system:
        let
          p = perSystem.${system};
        in
        {
          default = p.grok-oss;
          # Alias: `nix shell .#just` -> locked nixpkgs just (bootstrap only).
          just = p.justPkg;
          inherit (p)
            grok-oss
            cargo-mem-guard
            ci-tools
            cargo-mem-guard-unwrapped
            workspace-cargo-quality
            workspace-cargo-named-test
            ;
        }
      );

      checks = forAllSystems (
        system:
        let
          p = perSystem.${system};
        in
        {
          inherit (p)
            grok-oss
            cargoCheck
            openrouter-credentials
            cargo-mem-guard
            cargo-mem-guard-tests
            ;
        }
      );

      apps = forAllSystems (
        system:
        let
          p = perSystem.${system};
        in
        {
          default = {
            type = "app";
            program = "${p.grok-oss}/bin/grok-oss";
          };
          cargo-mem-guard = {
            type = "app";
            program = "${p.cargo-mem-guard}/bin/cargo-mem-guard";
          };
        }
      );

      devShells = forAllSystems (
        system:
        let
          p = perSystem.${system};
        in
        {
          default = p.devShell;
          ci = p.ciShell;
        }
      );
    };
}
