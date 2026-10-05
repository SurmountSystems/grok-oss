//! Process-isolated: missing GROK_EXTRA_CA_BUNDLE path → fail-open client build.

#[test]
fn missing_bundle_path_builds_clients_without_panic() {
    // Safety: sole test in this binary; set before any OnceLock resolve.
    unsafe {
        std::env::set_var(
            xai_grok_extra_ca::ENV_GROK_EXTRA_CA_BUNDLE,
            "/nonexistent/grok-extra-ca-bundle-invalid-file.pem",
        );
    }

    assert!(xai_grok_extra_ca::extra_root_ders().is_empty());

    xai_grok_extra_ca::build_reqwest_client(|builder| builder)
        .expect("async client builds when bundle is unreadable");

    xai_grok_extra_ca::build_blocking_reqwest_client(|builder| builder)
        .expect("blocking client builds when bundle is unreadable");
}
