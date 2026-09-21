{
  fenix,
  system,
}:
# Match rust-toolchain.toml (channel stable = 1.98.1 + clippy/rustfmt).
# 1.98.1 fixes vtable miscompilation (https://blog.rust-lang.org/2026/09/03/Rust-1.98.1/).
# FOD SRI for channel-rust-stable.toml: when rust-lang rewrites the
# manifest, just check fails with hash mismatch. Set sha256 to the
# "got:" value from the error.
fenix.packages.${system}.fromToolchainFile {
  file = ../rust-toolchain.toml;
  sha256 = "sha256-p8h3Sl/YRByZfZTAKXdsvF6xEenXKrXSVvpphmZENH4=";
}
