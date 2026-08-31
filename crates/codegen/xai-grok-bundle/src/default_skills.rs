//! In-tree Grok OSS default skills.
//!
//! Source of truth is this crate's `skills/` directory (compiled in with
//! `include_str!`). On startup, and after network bundle extract, Grok
//! writes those files into `<grok home>/bundled/skills/`. Discovery already
//! scans that tree as `Bundled` scope (`list_skills_with_options`).
//!
//! These files are **not** listed in the network `manifest.json`. A later
//! cli-chat-proxy extract therefore does not prune them as removed archive
//! entries. User edits (checksum mismatch vs every shipped body) are kept.

use anyhow::Result;
use std::path::Path;

use super::{checksum_bytes, checksum_file_if_exists, ensure_bundle_dirs, write_bundle_file};

/// Skill directory names shipped as Grok OSS defaults.
pub const DEFAULT_PRODUCT_SKILL_NAMES: &[&str] = &["polish", "subagent", "what"];

struct DefaultProductSkillFile {
    relative_path: &'static str,
    content: &'static str,
    /// Older shipped bodies we may overwrite on upgrade. Empty on first land.
    previous_checksums: &'static [&'static str],
}

const DEFAULT_PRODUCT_SKILL_FILES: &[DefaultProductSkillFile] = &[
    DefaultProductSkillFile {
        relative_path: "skills/polish/SKILL.md",
        content: include_str!("../skills/polish/SKILL.md"),
        previous_checksums: &[],
    },
    DefaultProductSkillFile {
        relative_path: "skills/polish/references/incident-classes.md",
        content: include_str!("../skills/polish/references/incident-classes.md"),
        previous_checksums: &[],
    },
    DefaultProductSkillFile {
        relative_path: "skills/subagent/SKILL.md",
        content: include_str!("../skills/subagent/SKILL.md"),
        previous_checksums: &[],
    },
    DefaultProductSkillFile {
        relative_path: "skills/what/SKILL.md",
        content: include_str!("../skills/what/SKILL.md"),
        previous_checksums: &[],
    },
];

/// Install Grok OSS default skills into `root` (`<grok home>/bundled`).
///
/// Writes when the path is missing, matches the current shipped body, or
/// matches a previous shipped body. Leaves any other on-disk bytes alone.
pub fn install_default_product_skills(root: &Path) -> Result<()> {
    ensure_bundle_dirs(root)?;
    for file in DEFAULT_PRODUCT_SKILL_FILES {
        install_one(root, file)?;
    }
    Ok(())
}

fn install_one(root: &Path, file: &DefaultProductSkillFile) -> Result<()> {
    let absolute_path = root.join(file.relative_path);
    let shipped = checksum_bytes(file.content.as_bytes());
    match checksum_file_if_exists(&absolute_path)? {
        None => write_bundle_file(&absolute_path, file.content.as_bytes())?,
        Some(on_disk) if on_disk == shipped => {}
        Some(on_disk) if file.previous_checksums.iter().any(|old| *old == on_disk) => {
            write_bundle_file(&absolute_path, file.content.as_bytes())?;
        }
        Some(_) => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn read(root: &Path, rel: &str) -> String {
        std::fs::read_to_string(root.join(rel)).unwrap()
    }

    #[test]
    fn default_product_skills_include_polish_and_subagent() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("skills");
        for name in DEFAULT_PRODUCT_SKILL_NAMES {
            let skill_md = root.join(name).join("SKILL.md");
            assert!(
                skill_md.is_file(),
                "default product skill {name} must exist at {}",
                skill_md.display()
            );
            let body = std::fs::read_to_string(&skill_md).unwrap();
            assert!(
                body.contains(&format!("name: {name}")),
                "{name} SKILL.md must declare its name in frontmatter"
            );
            assert!(
                body.contains("default Grok OSS skill"),
                "{name} must say it is a default Grok OSS skill"
            );
            assert!(
                body.contains(&format!("bundled/skills/{name}")),
                "{name} must name the bundled install path"
            );
        }
        assert!(
            root.join("polish/references/incident-classes.md").is_file(),
            "polish must ship incident-classes.md"
        );
    }

    #[test]
    fn install_writes_polish_and_subagent_into_bundled_skills() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        install_default_product_skills(root).unwrap();
        assert!(
            read(root, "skills/polish/SKILL.md").contains("name: polish"),
            "install must write polish"
        );
        assert!(
            read(root, "skills/subagent/SKILL.md").contains("name: subagent"),
            "install must write subagent"
        );
        assert!(
            read(root, "skills/what/SKILL.md").contains("name: what"),
            "install must write what"
        );
        assert!(
            read(root, "skills/polish/references/incident-classes.md").contains("Incident classes"),
            "install must write polish references"
        );
        assert!(
            !root.join("manifest.json").exists(),
            "default product skills must not join the network bundle manifest"
        );
    }

    #[test]
    fn install_does_not_overwrite_user_edits() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        install_default_product_skills(root).unwrap();
        let path = root.join("skills/polish/SKILL.md");
        std::fs::write(&path, "user customized polish\n").unwrap();
        install_default_product_skills(root).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "user customized polish\n"
        );
    }

    #[test]
    fn install_overwrites_when_previous_checksum_matches() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let old_body = "old shipped polish\n";
        let path = root.join("skills/polish/SKILL.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, old_body).unwrap();
        let old_checksum = checksum_bytes(old_body.as_bytes());
        let leaked: &'static str = Box::leak(old_checksum.into_boxed_str());
        let previous: &'static [&'static str] = Box::leak(Box::new([leaked]));
        let file = DefaultProductSkillFile {
            relative_path: "skills/polish/SKILL.md",
            content: include_str!("../skills/polish/SKILL.md"),
            previous_checksums: previous,
        };
        install_one(root, &file).unwrap();
        assert!(
            read(root, "skills/polish/SKILL.md").contains("name: polish"),
            "previous shipped body must upgrade"
        );
    }
}
