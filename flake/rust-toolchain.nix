{
  fenix,
  system,
}:
# Match rust-toolchain.toml (channel stable = 1.98.0 + clippy/rustfmt).
# FOD SRI for channel-rust-stable.toml: when rust-lang rewrites the
# manifest, just check fails with hash mismatch. Set sha256 to the
# "got:" value from the error.
fenix.packages.${system}.fromToolchainFile {
  file = ../rust-toolchain.toml;
  sha256 = "sha256-P30Tm3O7vQAE725YtDCDHGjNrSsfZO4us11UwJGZSJo=";
}
