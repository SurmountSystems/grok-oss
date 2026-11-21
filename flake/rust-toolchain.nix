{
  fenix,
  system,
}:
# Match rust-toolchain.toml (channel = "stable", plus clippy and rustfmt).
# Stable moved to 1.100.0 on 2026-11-12
# (https://blog.rust-lang.org/2026/11/12/Rust-1.100.0/, accessed: 2026-11-15).
# FOD SRI is channel-rust-stable.toml. When rust-lang rewrites the manifest,
# just ci fails with hash mismatch. Set sha256 to the "got:" value from the
# error. Hash updated 2026-11-15 from GitHub Actions run 43732732771
# (drv channel-rust-stable.toml).
fenix.packages.${system}.fromToolchainFile {
  file = ../rust-toolchain.toml;
  sha256 = "sha256-MFHoWVcPZKjFrLnQnodCXbUsfZcofGfo9tV9fTTvklg=";
}
