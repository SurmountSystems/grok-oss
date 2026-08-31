{
  description = "Grok OSS - unofficial open-source fork of xAI Grok Build";

  # Input surface is intentionally small (no flake-utils / systems).
  # github: still uses the tarball API, but with fewer inputs and NIX_CONFIG
  # download-attempts + just nix_retry we survive free-GHA 502/503s.
  # Avoid git+https for nixpkgs: a full clone is multi-GB and more fragile
  # on free runners than a single tarball of the locked rev.
  # Output bodies live under flake/*.nix (same packages and checks).
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
      systems = import ./flake/systems.nix;
      forAllSystems = nixpkgs.lib.genAttrs systems;

      perSystem = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (pkgs) lib;

          rustToolchain = import ./flake/rust-toolchain.nix {
            inherit fenix system;
          };

          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          memGuard = import ./flake/cargo-mem-guard.nix {
            inherit pkgs lib craneLib;
          };

          nixHelper = import ./flake/grok-nix-helper.nix {
            inherit pkgs lib craneLib;
          };

          grokOss = import ./flake/grok-oss.nix {
            inherit
              pkgs
              lib
              craneLib
              self
              ;
          };

          quality = import ./flake/workspace-quality.nix {
            inherit pkgs lib craneLib;
            inherit (grokOss) commonArgs nativeBuildInputs buildInputs;
          };

          namedTest = import ./flake/workspace-named-test.nix {
            inherit pkgs lib craneLib;
            inherit (grokOss) commonArgs nativeBuildInputs buildInputs;
            inherit (quality) workspaceCargoArtifacts workspaceCargoJobsFromCores;
          };

          ci = import ./flake/ci-shell.nix {
            inherit pkgs lib rustToolchain;
            inherit (memGuard) cargo-mem-guard;
            inherit (nixHelper) grok-nix-helper;
          };
        in
        {
          inherit (grokOss)
            grok-oss
            cargoCheck
            openrouter-credentials
            ;
          inherit (memGuard)
            cargo-mem-guard
            cargo-mem-guard-unwrapped
            cargo-mem-guard-tests
            ;
          inherit (nixHelper)
            grok-nix-helper
            grok-nix-helper-unwrapped
            grok-nix-helper-tests
            ;
          inherit (quality) workspace-cargo-quality;
          inherit (namedTest) workspace-cargo-named-test;
          inherit (ci)
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
            grok-nix-helper
            grok-nix-helper-unwrapped
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
            grok-nix-helper
            grok-nix-helper-tests
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
          grok-nix-helper = {
            type = "app";
            program = "${p.grok-nix-helper}/bin/grok-nix-helper";
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
