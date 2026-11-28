//! Rust `bulk_edit_policy` is the required path. This bulk-edit path must not spawn python3.

use std::path::PathBuf;

fn hook_path() -> PathBuf {
    if let Ok(grok_home) = std::env::var("GROK_HOME") {
        if !grok_home.is_empty() {
            return PathBuf::from(grok_home).join("hooks/block-bulk-replace-edit.py");
        }
    }
    let home = std::env::var("HOME").expect("HOME");
    PathBuf::from(home).join(".grok/hooks/block-bulk-replace-edit.py")
}

/// Code before a `#` comment. A shebang is not passed here.
fn code_without_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or("").trim()
}

#[test]
fn bulk_edit_policy_is_the_required_path_and_must_not_spawn_python3() {
    let path = hook_path();
    if path.is_file() {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        let shebang = text.lines().next().unwrap_or("");
        assert!(
            !shebang.contains("python"),
            "bulk-edit path must not spawn python3; shebang is {shebang}"
        );
        for line in text.lines().skip(1) {
            let code = code_without_comment(line);
            assert!(
                !code.contains("python"),
                "bulk-edit path must not spawn python3: {line}"
            );
        }
    }

    let search_replace = include_str!("../src/implementations/grok_build/search_replace/mod.rs");
    assert!(
        search_replace.contains("bulk_edit_policy::evaluate"),
        "Rust bulk_edit_policy is the required path"
    );
}
