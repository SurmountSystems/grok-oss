//! Prompt-template codegen is the `encrypt_templates` Rust bin in this crate.
//! The job runs as Rust and does not require python3.

use std::path::Path;

#[test]
fn encrypt_templates_codegen_runs_as_rust_and_does_not_require_python3() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let python_codegen = root.join("scripts/encrypt_templates.py");
    assert!(
        !python_codegen.exists(),
        "the job runs as Rust and does not require python3; found {}",
        python_codegen.display()
    );

    let template_rs = std::fs::read_to_string(root.join("src/prompt/template.rs"))
        .expect("read src/prompt/template.rs");
    assert!(
        !template_rs.contains("python3") && !template_rs.contains("encrypt_templates.py"),
        "the job runs as Rust and does not require python3"
    );

    let rust_bin = root.join("src/bin/encrypt_templates.rs");
    assert!(
        rust_bin.is_file(),
        "the job runs as Rust and does not require python3; missing {}",
        rust_bin.display()
    );
    let bin_rs = std::fs::read_to_string(&rust_bin).expect("read encrypt_templates.rs");
    assert!(
        bin_rs.contains("fn xor_encrypt") && bin_rs.contains("fn main"),
        "the job runs as Rust and does not require python3"
    );
    assert!(
        bin_rs.contains("the encrypt_templates Rust bin"),
        "the job runs as Rust and does not require python3"
    );
    assert!(
        !bin_rs.contains("python3 scripts/encrypt_templates.py"),
        "the job runs as Rust and does not require python3"
    );
}
