//! Regenerate `src/prompt/prompt_encrypted.rs` from `templates/`.
//!
//! Rust replacement for `scripts/encrypt_templates.py`. grok-oss does not
//! spawn python3 for this helper. Run the `encrypt_templates` bin in this
//! crate. Do not use python3.

use std::fs;
use std::path::PathBuf;

const SEEDS: [(&str, u8, &str); 3] = [
    ("BASE_PROMPT_ENC", 0x5A, "prompt.md"),
    ("CODEX_PROMPT_ENC", 0x7B, "apply_patch_prompt.md"),
    ("SUBAGENT_PROMPT_ENC", 0x3D, "subagent_prompt.md"),
];

fn xor_encrypt(data: &[u8], seed: u8) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, &b)| b ^ seed.wrapping_add(i as u8))
        .collect()
}

fn main() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let template_dir = crate_dir.join("templates");
    let out_path = crate_dir.join("src/prompt/prompt_encrypted.rs");

    let mut lines = vec![
        "// Auto-generated -- do not edit.".to_string(),
        "// Regenerate: the encrypt_templates Rust bin in this crate. Do not use python3."
            .to_string(),
        "// XOR-encrypted prompt templates (key = position-dependent seed).".to_string(),
        String::new(),
    ];
    for (const_name, seed, filename) in SEEDS {
        let path = template_dir.join(filename);
        let data = fs::read(&path).unwrap_or_else(|err| {
            panic!("read {}: {err}", path.display());
        });
        let enc = xor_encrypt(&data, seed);
        let arr = enc
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push("#[rustfmt::skip]".to_string());
        lines.push(format!("pub(crate) const {const_name}: &[u8] = &[{arr}];"));
        lines.push(String::new());
    }
    let seeds_arr = SEEDS
        .iter()
        .map(|(_, seed, _)| format!("0x{seed:02X}"))
        .collect::<Vec<_>>()
        .join(", ");
    lines.push(format!(
        "pub(crate) const PROMPT_SEEDS: [u8; {}] = [{seeds_arr}];",
        SEEDS.len()
    ));
    lines.push(String::new());

    fs::write(&out_path, lines.join("\n")).unwrap_or_else(|err| {
        panic!("write {}: {err}", out_path.display());
    });
    println!("Wrote {}", out_path.display());
}
