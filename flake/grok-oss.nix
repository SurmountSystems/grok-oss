# Preserved flake outputs: grok-oss (packages.default / checks / apps.default),
# cargoCheck and openrouter-credentials (checks).
# Also exports src, nativeBuildInputs, buildInputs, commonArgs, cargoArtifacts
# for flake/workspace-quality.nix and flake/workspace-named-test.nix.
{
  pkgs,
  lib,
  craneLib,
  self,
}:
let
  src = lib.cleanSourceWith {
    src = ../.;
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
        cargoToml = ../crates/codegen/xai-grok-pager-bin/Cargo.toml;
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
in
{
  inherit
    src
    nativeBuildInputs
    buildInputs
    commonArgs
    cargoArtifacts
    grok-oss
    cargoCheck
    openrouter-credentials
    ;
}
