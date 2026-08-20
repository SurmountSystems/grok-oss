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
              || base == "rust-toolchain.toml"
              || base == "clippy.toml"
              || base == "rustfmt.toml"
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

          # Full workspace cargo fmt, clippy, and test compile. `just check-remote`
          # realizes this on the Nix remote builder. `--no-run` compiles tests
          # without executing them in the sandbox. Not in `checks` so
          # `nix flake check` / GHA stay local and do not need the builder.
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
          # capped at 32. If the sandbox reports fewer cores than
          # CARGO_BUILD_JOBS, keep 32 so a 1-core NIX_BUILD_CORES does not
          # force one rustc. cargo fmt rejects --jobs; only clippy/check/
          # build/test get it.
          workspaceCargoJobsFromCores = ''
            cargoJobs="''${NIX_BUILD_CORES:-32}"
            case "$cargoJobs" in
              "" | *[!0-9]*) cargoJobs=32 ;;
            esac
            if [ "$cargoJobs" -gt 32 ]; then
              cargoJobs=32
            fi
            if [ "''${CARGO_BUILD_JOBS:-0}" -gt "$cargoJobs" ]; then
              cargoJobs="$CARGO_BUILD_JOBS"
            fi
            export CARGO_BUILD_JOBS="$cargoJobs"
          '';
          workspaceCargoArtifacts = craneLib.buildDepsOnly (
            commonArgs
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
                cargoWithProfile check --jobs "$CARGO_BUILD_JOBS" --locked --all-targets
                cargoWithProfile build --jobs "$CARGO_BUILD_JOBS" --locked
              '';
              checkPhaseCargoCommand = ''
                ${workspaceCargoJobsFromCores}
                cargoWithProfile test --jobs "$CARGO_BUILD_JOBS" --locked --no-run
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
              buildPhaseCargoCommand = ''
                ${workspaceCargoJobsFromCores}
                cargo fmt --all -- --check
                cargoWithProfile clippy --jobs "$CARGO_BUILD_JOBS" --workspace --lib --bins --locked -- -D warnings
                cargoWithProfile test --jobs "$CARGO_BUILD_JOBS" --workspace --locked --no-run
                # cargo 1.97 refuses --doc with --no-run ("can't skip running
                # doc tests with --no-run"). Doctest *execution* stays out of
                # this sandbox; rustdoc examples are not compile-only here.
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
