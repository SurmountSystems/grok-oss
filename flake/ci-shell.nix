# Preserved flake outputs: ci-tools, just (packages.just from justPkg),
# devShells.default, devShells.ci.
{
  pkgs,
  lib,
  rustToolchain,
  cargo-mem-guard,
  grok-nix-helper,
}:
let
  # Host CI toolchain (free GHA / low-mem)
  #
  # A single buildEnv so consumers can:
  #   nix shell .#ci-tools -c cargo-mem-guard -- cargo check ...
  #   nix develop .#ci
  # without assembling PATH by hand or writing bash wrappers.
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
  # (`grok-nix-helper hermetic-path`); git + python3 must live here
  # so scrub does not drop VCS (cargo git deps / git unit tests) or the
  # interpreter (cgroup_memory_test + mock LSP e2e spawn `python3`).
  ci-tools = pkgs.buildEnv {
    name = "grok-oss-ci-tools";
    paths =
      [
        rustToolchain
        cargo-mem-guard
        grok-nix-helper
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
      description = "Host CI toolchain: fenix rustc, cargo-nextest, cargo-mem-guard, grok-nix-helper, mold, git, python3, build deps";
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
      grok-nix-helper
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
      echo "  nix build .#grok-nix-helper"
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
    justPkg
    ci-tools
    ciLowMemEnv
    devShell
    ciShell
    ;
}
