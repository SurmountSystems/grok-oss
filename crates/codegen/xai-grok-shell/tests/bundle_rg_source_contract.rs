//! Bundled rg is cargo-built from the ripgrep crate; GitHub musl tarball is
//! not the install path.

#[test]
fn bundled_rg_is_cargo_built_from_the_ripgrep_crate_github_musl_tarball_is_not_the_install_path() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs"));
    assert!(
        src.contains("cargo")
            && src.contains("install")
            && src.contains("ripgrep")
            && src.contains("--bin")
            && src.contains("rg"),
        "bundled rg is cargo-built from the ripgrep crate; GitHub musl tarball is not the install path"
    );
    assert!(
        !src.contains("github.com/BurntSushi/ripgrep/releases")
            && !src.contains("ripgrep/releases/download"),
        "xai-grok-shell build.rs must not download a GitHub musl ripgrep tarball"
    );
    assert!(
        !src.contains("GROK_SHELL_RG_TARGET=x86_64-unknown-linux-musl"),
        "xai-grok-shell build.rs must not default GROK_SHELL_RG_TARGET to musl"
    );
}
