//! grok-oss grep is embedded in `xai-grok-tools` (`grep` crate + `ignore`).
//! This crate does not cargo-install ripgrep and does not bundle a sidecar `rg`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
