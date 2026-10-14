{
  fenix,
  system,
}:
# Match rust-toolchain.toml (channel stable = 1.99.0 + clippy/rustfmt).
# 1.99.0 is current stable (https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/,
# accessed: 2026-10-03). FOD SRI is channel-rust-stable.toml, byte-identical
# to channel-rust-1.99.0.toml on that date. When rust-lang rewrites the
# manifest, just check fails with hash mismatch. Set sha256 to the
# "got:" value from the error.
fenix.packages.${system}.fromToolchainFile {
  file = ../rust-toolchain.toml;
  sha256 = "sha256-zm3dyIY2T414ZRR3EhLOvptzG6gta4WZUcawzMUWtqI=";
}
