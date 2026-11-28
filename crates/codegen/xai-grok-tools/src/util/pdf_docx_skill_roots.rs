//! Install PDF and DOCX Rust skill trees when the process home does not
//! already have them.
//!
//! The python3 checks read `$HOME/.agents/skills/pdf/scripts`,
//! `$HOME/.grok/bundled/skills/pdf/scripts`, `$HOME/.agents/skills/docx`,
//! and `$HOME/.grok/bundled/skills/docx`. A builder that does not have
//! those directories still gets the shipped Rust sources before those
//! checks run. An existing directory is left unchanged. This module does
//! not start an interpreter and does not write a `.py` file.

use std::path::{Path, PathBuf};

/// Create each missing home skill directory from the shipped Rust tree.
pub fn ensure_pdf_docx_skill_roots() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=skill-trees");
    println!("cargo:rerun-if-env-changed=HOME");
    install_if_absent(".agents/skills/pdf/scripts", "skill-trees/pdf/scripts")?;
    install_if_absent(
        ".grok/bundled/skills/pdf/scripts",
        "skill-trees/pdf/scripts",
    )?;
    install_if_absent(".agents/skills/docx", "skill-trees/docx")?;
    install_if_absent(".grok/bundled/skills/docx", "skill-trees/docx")?;
    Ok(())
}

fn install_if_absent(home_rel: &str, shipped_rel: &str) -> std::io::Result<()> {
    let Some(dest) = home_join(home_rel) else {
        return Ok(());
    };
    if dest.is_dir() {
        return Ok(());
    }
    copy_dir(&shipped(shipped_rel), &dest)
}

fn home_join(rel: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    if home.is_empty() {
        return None;
    }
    Some(PathBuf::from(home).join(rel))
}

fn shipped(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn copy_dir(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
