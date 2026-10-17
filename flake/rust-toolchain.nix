{
  fenix,
  system,
}:
# Match rust-toolchain.toml (channel stable = 1.99.0 + clippy/rustfmt).
# 1.99.0 is current stable (https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/,
# accessed: 2026-10-03). FOD SRI is channel-rust-stable.toml. When rust-lang
# rewrites the manifest, just check fails with hash mismatch. Set sha256 to
# the "got:" value from the error. Hash updated 2026-10-17 from GHA job
# 117261020162 (drv channel-rust-stable.toml).
fenix.packages.${system}.fromToolchainFile {
  file = ../rust-toolchain.toml;
  sha256 = "sha256-+xUr6Al1wZSqX8aEsq7DpuFOK4Ve/i+qTl+K5uWQ9GM=";
}
