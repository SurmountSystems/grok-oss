//! DOCX skill work must not spawn python3.

use std::fs;
use std::path::Path;

fn invokes_python(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("python3")
        || lower.contains("python2")
        || lower.contains("#!/usr/bin/env python")
        || lower.contains("command::new(\"python")
}

fn collect_py(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_py(&path, out);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("py") {
            out.push(path.display().to_string());
        }
    }
}

#[test]
fn docx_work_must_not_spawn_python3() {
    let home = std::env::var("HOME").expect("HOME");
    let roots = [
        format!("{home}/.agents/skills/docx"),
        format!("{home}/.grok/bundled/skills/docx"),
    ];
    let mut python_scripts = Vec::new();
    for root in &roots {
        let root = Path::new(root);
        assert!(
            root.is_dir(),
            "DOCX work must not spawn python3; missing {}",
            root.display()
        );
        collect_py(root, &mut python_scripts);
        for rel in ["docx-skill/src/main.rs", "docx-skill/src/xml.rs"] {
            let path = root.join(rel);
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
            assert!(
                !invokes_python(&text),
                "DOCX work must not spawn python3: {}",
                path.display()
            );
        }
    }
    assert!(
        python_scripts.is_empty(),
        "DOCX work must not spawn python3; still python: {}",
        python_scripts.join(", ")
    );
}
