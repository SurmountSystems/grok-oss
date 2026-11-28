//! The recording stdio helper is a built binary, not an interpreter command.

use std::path::Path;

#[test]
fn recording_mcp_server_bin_is_built() {
    let bin = env!("CARGO_BIN_EXE_recording_mcp_server");
    assert!(
        Path::new(bin).is_file(),
        "recording helper binary missing at {bin}"
    );
    let mcp = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mcp.rs");
    let text = std::fs::read_to_string(&mcp).expect("read mcp.rs");
    assert!(!text.contains("python3"), "src/mcp.rs still names python3");
}
