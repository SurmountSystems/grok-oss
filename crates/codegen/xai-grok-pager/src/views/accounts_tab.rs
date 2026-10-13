//! Accounts tab. Every stored login is a row. The next request is marked.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// Stored logins under `$GROK_HOME`, or the process grok home when that
/// variable is unset. A missing auth file is an empty list.
pub(crate) fn accounts_tab_lines_from_home() -> Vec<String> {
    let home = match std::env::var("GROK_HOME") {
        Ok(home) => std::path::PathBuf::from(home),
        Err(_) => xai_grok_shell::util::grok_home::grok_home(),
    };
    let Ok(map) = xai_grok_shell::auth::read_auth_json(&home.join("auth.json")) else {
        return Vec::new();
    };
    let pins = xai_grok_shell::auth::limits_pins::load_limits_pins();
    xai_grok_shell::auth::accounts_tab_lines(&map, pins.supergrok_identity, pins.use_console)
}

/// Paint the Accounts tab into `area`, one line per row, starting at the top.
pub fn render_accounts_tab(buf: &mut Buffer, area: Rect, lines: &[String]) {
    for (index, line) in lines.iter().enumerate() {
        let y = area.y.saturating_add(index as u16);
        if y >= area.y.saturating_add(area.height) {
            break;
        }
        buf.set_string(area.x, y, line, Style::default());
    }
}

#[cfg(test)]
mod tests {
    use super::render_accounts_tab;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use xai_grok_shell::auth::{
        AuthMode, GrokAuth, SupergrokIdentityPin, accounts_tab_lines, upsert_supergrok_session,
    };

    fn painted(lines: &[String]) -> String {
        let area = Rect::new(0, 0, 80, 12);
        let mut buf = Buffer::empty(area);
        render_accounts_tab(&mut buf, area, lines);
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn accounts_tab_lists_every_stored_oauth_identity_and_the_console_api_key() {
        let base = "https://auth.x.ai::accounts-tab";
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "personal-token-do-not-print".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "536d3388-0735-4e09-bf46-a33da0ed54aa".into(),
                email: Some("cryptoquick@gmail.com".into()),
                team_id: Some("58c5f686-4270-4d6d-9c3b-df44559f8457".into()),
                principal_type: Some("User".into()),
                ..GrokAuth::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "business-token-do-not-print".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "8b6f008a-5a53-474a-a531-9a20525d66f5".into(),
                email: Some("hunter@surmount.systems".into()),
                refresh_token: Some("refresh-do-not-print".into()),
                principal_type: Some("Team".into()),
                team_id: Some("61fab250-b2c1-40cf-b5b8-628e673a2eeb".into()),
                team_name: Some("Surmount".into()),
                team_role: Some("ADMIN".into()),
                ..GrokAuth::default()
            },
        );
        map.insert(
            "xai::api_key".into(),
            GrokAuth {
                key: "console-key-do-not-print".into(),
                auth_mode: AuthMode::ApiKey,
                user_id: String::new(),
                ..GrokAuth::default()
            },
        );
        let lines = accounts_tab_lines(&map, Some(SupergrokIdentityPin::Business), false);
        let tab = painted(&lines);
        assert!(
            tab.contains("Accounts"),
            "the Accounts tab title is on the card:\n{tab}"
        );
        assert!(
            tab.contains("cryptoquick@gmail.com - personal"),
            "a business pin still lists the personal login:\n{tab}"
        );
        assert!(
            tab.contains("hunter@surmount.systems - business (current)"),
            "the next request uses the business login:\n{tab}"
        );
        assert!(
            !tab.contains("cryptoquick@gmail.com - personal (current)"),
            "the personal login stays listed and is not the next request:\n{tab}"
        );
        assert!(
            tab.contains("console API key"),
            "the stored console API key is its own row:\n{tab}"
        );
        assert!(
            tab.contains("3 sessions"),
            "three stored accounts count as 3 sessions:\n{tab}"
        );
        assert!(
            !tab.contains("1 session"),
            "the caption counts every stored account:\n{tab}"
        );
        for secret in [
            "personal-token-do-not-print",
            "business-token-do-not-print",
            "refresh-do-not-print",
            "console-key-do-not-print",
        ] {
            assert!(
                !tab.contains(secret),
                "the Accounts tab does not print {secret}:\n{tab}"
            );
        }
    }
}
