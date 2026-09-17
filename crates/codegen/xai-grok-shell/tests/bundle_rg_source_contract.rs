//! grok-oss grep is embedded Rust, not a sidecar `rg`.
//! `just install` does not cargo-install ripgrep.

#[test]
fn grok_oss_grep_is_embedded_rust_not_a_sidecar_rg() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs"));
    assert!(
        !src.contains("cargo install") && !src.contains("arg(\"ripgrep\")"),
        "grok-oss grep is embedded Rust, not a sidecar rg: xai-grok-shell build.rs must not cargo-install ripgrep"
    );
    assert!(
        !src.contains("github.com/BurntSushi/ripgrep/releases")
            && !src.contains("ripgrep/releases/download"),
        "xai-grok-shell build.rs must not download a GitHub musl ripgrep tarball"
    );
    assert!(
        src.contains("embedded") || src.contains("does not cargo-install ripgrep"),
        "xai-grok-shell build.rs must say grok-oss grep is embedded, not a sidecar rg"
    );
}
