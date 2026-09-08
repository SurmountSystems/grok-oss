pub const PAGER_CLIENT_TYPE: &str = "grok-pager";
pub const HEADLESS_CLIENT_TYPE: &str = "grok-shell";

pub const PAGER_CLIENT_VERSION: &str = xai_grok_version::VERSION;

/// User-facing product binary name for Surmount **Grok OSS**.
///
/// Shown in terminal/tab titles, resume hints, and CLI branding. Upstream xAI
/// uses bare `grok`; this fork's install artifact is `grok-oss`. Config keys
/// that refer to the brand slot (e.g. title item `"grok"`) may keep the short
/// name for compatibility. The display string is always this constant.
pub const PRODUCT_CLI_NAME: &str = "grok-oss";

/// Operator-facing `--version` line. First token is always [`PRODUCT_CLI_NAME`].
///
/// Example: `grok-oss 1.0.3 (f1abb5fd33b6)`. Never print bare `grok` as the
/// product token. `version_with_commit` is the compiled version + git SHA
/// (`VERSION_WITH_COMMIT`); `channel_label` is the optional suffix from
/// `xai_grok_update::channel_label()`.
pub fn product_version_line(version_with_commit: &str, channel_label: &str) -> String {
    format!(
        "{PRODUCT_CLI_NAME} {}",
        xai_grok_version::display_version_with_commit(version_with_commit, channel_label)
    )
}

/// Pasteable `Resume this session with:` command for this product.
pub fn resume_session_command(session_id: &str, minimal: bool) -> String {
    if minimal {
        format!("{PRODUCT_CLI_NAME} --minimal --resume {session_id}")
    } else {
        format!("{PRODUCT_CLI_NAME} --resume {session_id}")
    }
}

/// `User-Agent` for pager-owned direct-to-`api.x.ai` clients (voice STT).
///
/// Matches the sampler's `grok-shell/<version> (os; arch)` shape so server-side
/// dashboards bucket voice traffic alongside chat / imagine requests.
pub fn client_user_agent() -> String {
    format!(
        "{}/{} ({}; {})",
        HEADLESS_CLIENT_TYPE,
        PAGER_CLIENT_VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_user_agent_has_expected_shape() {
        // e.g. "grok-shell/1.2.3 (macos; aarch64)". The pieces are wire
        // contract for server-side UA parsing, so pin the exact shape.
        let ua = client_user_agent();
        assert_eq!(
            ua,
            format!(
                "grok-shell/{} ({}; {})",
                PAGER_CLIENT_VERSION,
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        );
    }

    #[test]
    fn product_cli_name_is_grok_oss() {
        assert_eq!(PRODUCT_CLI_NAME, "grok-oss");
        // Must not regress to bare upstream brand in product-facing surfaces.
        assert_ne!(PRODUCT_CLI_NAME, "grok");
    }

    #[test]
    fn product_version_line_uses_grok_oss_not_bare_grok() {
        // Operator report: `grok 1.0.3 (f1abb5fd33b6)` is the wrong product token.
        let line = product_version_line("1.0.3 (f1abb5fd33b6)", "");
        assert_eq!(line, "grok-oss 1.0.3 (f1abb5fd33b6)");
        assert_eq!(line.split_whitespace().next(), Some("grok-oss"));
        assert_ne!(line.split_whitespace().next(), Some("grok"));
    }

    #[test]
    fn resume_session_command_uses_grok_oss() {
        assert_eq!(resume_session_command("01", false), "grok-oss --resume 01");
        assert_eq!(
            resume_session_command("01", true),
            "grok-oss --minimal --resume 01"
        );
    }

    /// Operator report 2026-09-01: quit / rebuild handoff taught `grok --resume`.
    /// Pasteable resume is always this product CLI, including when argv0 is `grok`.
    #[test]
    fn resume_session_command_never_teaches_bare_grok_resume() {
        let id = "01a027e0-20ad-7a62-ab05-5d65b99e34b1";
        let full = resume_session_command(id, false);
        let minimal = resume_session_command(id, true);
        assert_eq!(full, format!("grok-oss --resume {id}"));
        assert_eq!(minimal, format!("grok-oss --minimal --resume {id}"));
        for out in [&full, &minimal] {
            assert!(
                !out.contains("grok --resume"),
                "must not tell operators to run upstream grok --resume: {out}"
            );
            assert!(
                out.starts_with("grok-oss "),
                "resume paste must start with grok-oss, got {out}"
            );
        }
    }
}
