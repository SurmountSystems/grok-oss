//! Launchers for in-scope CI and test helpers must not name python3.

use std::path::{Path, PathBuf};

fn repo_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn assert_no_python3(path: &Path) {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    assert!(
        !text.contains("python3"),
        "{} still names python3",
        path.display()
    );
}

#[test]
fn ci_helpers_do_not_require_python3() {
    // These env values make cargo build the helper binaries for this test.
    let _built = [
        env!("CARGO_BIN_EXE_cgroup_memory_alloc"),
        env!("CARGO_BIN_EXE_lsp_mock_server"),
    ];
    let files = [
        repo_path("tests/cgroup_memory_test.rs"),
        repo_path("src/implementations/lsp/tests.rs"),
        repo_path("src/bin/cgroup_memory_alloc.rs"),
        repo_path("src/bin/lsp_mock_server.rs"),
        repo_path("../xai-grok-workspace/src/mcp.rs"),
        repo_path("../xai-grok-workspace/src/bin/recording_mcp_server.rs"),
        repo_path("../xai-grok-shell/src/bin/memory_integration_tests.rs"),
        repo_path("../../../justfile"),
    ];
    for path in files {
        assert_no_python3(&path);
    }
}
