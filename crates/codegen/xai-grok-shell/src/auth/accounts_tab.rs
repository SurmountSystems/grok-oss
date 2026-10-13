//! Accounts tab rows for every stored login.
//!
//! A business pin does not drop the personal login. A stored console API key
//! is its own row. The row the next request uses ends with ` (current)`.
//! Emails are labels. Tokens, refresh tokens, and API keys stay off the list.

use super::limits_pins::oauth_email_for_next_request;
use super::model::{API_KEY_SCOPE, AuthStore};
use super::supergrok_identity_rank::SupergrokIdentityPin;

struct AccountRow {
    order: u8,
    label: String,
    email: Option<String>,
    role: &'static str,
    current: bool,
}

/// Lines for the Accounts tab.
///
/// Order is personal, then each business login, then the console API key
/// when that credential is stored. An empty store returns no lines. The
/// base mirror is not an extra row. `pin` and `use_console` choose the
/// `(current)` row. They do not hide the other stored logins.
pub fn accounts_tab_lines(
    map: &AuthStore,
    pin: Option<SupergrokIdentityPin>,
    use_console: bool,
) -> Vec<String> {
    let mut rows = Vec::new();
    for listing in super::dual_auth_status::listings_including_lone_base_session(map) {
        let role = listing.role_label;
        let order = if role == "business" { 1 } else { 0 };
        let email = listing.email.filter(|email| !email.is_empty());
        let label = match email.as_deref() {
            Some(email) => format!("{email} - {role}"),
            None => role.to_string(),
        };
        rows.push(AccountRow {
            order,
            label,
            email,
            role,
            current: false,
        });
    }
    let console_stored = map
        .get(API_KEY_SCOPE)
        .is_some_and(|auth| !auth.key.trim().is_empty());
    if console_stored {
        rows.push(AccountRow {
            order: 2,
            label: "console API key".to_string(),
            email: None,
            role: "console",
            current: false,
        });
    }
    if rows.is_empty() {
        return Vec::new();
    }
    rows.sort_by_key(|left| left.order);
    mark_next_request_row(&mut rows, map, pin, use_console);
    let count = rows.len();
    let mut lines = Vec::with_capacity(count + 2);
    lines.push("Accounts".to_string());
    for row in rows {
        if row.current {
            lines.push(format!("{} (current)", row.label));
        } else {
            lines.push(row.label);
        }
    }
    let caption = if count == 1 {
        "1 session".to_string()
    } else {
        format!("{count} sessions")
    };
    lines.push(caption);
    lines
}

fn mark_next_request_row(
    rows: &mut [AccountRow],
    map: &AuthStore,
    pin: Option<SupergrokIdentityPin>,
    use_console: bool,
) {
    if use_console {
        if let Some(row) = rows.iter_mut().find(|row| row.role == "console") {
            row.current = true;
        }
        return;
    }
    let next_email = oauth_email_for_next_request(map, pin);
    if let Some(email) = next_email.as_deref()
        && let Some(row) = rows
            .iter_mut()
            .find(|row| row.role != "console" && row.email.as_deref() == Some(email))
    {
        row.current = true;
        return;
    }
    let want = if pin == Some(SupergrokIdentityPin::Business) {
        "business"
    } else {
        "personal"
    };
    if let Some(row) = rows.iter_mut().find(|row| row.role == want) {
        row.current = true;
    }
}
