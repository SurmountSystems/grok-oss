//! `/blacklist <command>` writes a machine-wide bash deny.
//!
//! `/blacklist lean` appends `Bash(lean)` and `Bash(lean *)` to
//! `[permission].deny` in the user `config.toml`. Deny is read at session
//! start and wins over allow and over always-approve.

use std::path::{Path, PathBuf};

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Blacklist one bare command on this machine.
pub struct BlacklistCommand;

impl SlashCommand for BlacklistCommand {
    fn name(&self) -> &str {
        "blacklist"
    }

    fn description(&self) -> &str {
        "Blacklist a command on this machine (deny it in every later session)"
    }

    fn usage(&self) -> &str {
        "/blacklist <command>"
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn args_required(&self) -> bool {
        true
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let path = match user_config_path() {
            Ok(path) => path,
            Err(err) => return CommandResult::Error(err),
        };
        match BlacklistCommand::record_at(&path, args) {
            Ok([bare, with_args]) => {
                let command = args.trim();
                CommandResult::Message(format!(
                    "Blacklisted `{command}` on this machine ({bare}, {with_args}). Bare `{command}` and `{command}` with arguments are denied. The deny applies on the next session."
                ))
            }
            Err(err) => CommandResult::Error(err),
        }
    }
}

impl BlacklistCommand {
    /// What `/blacklist <command>` writes, against an explicit config path.
    pub(crate) fn record_at(path: &Path, args: &str) -> Result<[String; 2], String> {
        append_bash_blacklist_at(path, args)
    }
}

fn user_config_path() -> Result<PathBuf, String> {
    let home = xai_grok_config::user_grok_home()
        .ok_or_else(|| "no grok home; cannot write a machine-wide deny".to_string())?;
    Ok(home.join(xai_grok_config::USER_CONFIG_FILENAME))
}

/// A single command word. Rejects globs and extra tokens so the deny cannot
/// match a different word that merely contains the same letters.
fn validate_blacklist_command(raw: &str) -> Result<&str, String> {
    let command = raw.trim();
    if command.is_empty() {
        return Err("/blacklist needs one command name, for example /blacklist lean".to_string());
    }
    if command.split_whitespace().nth(1).is_some() {
        return Err(
            "/blacklist takes one command name, not arguments. Example: /blacklist lean"
                .to_string(),
        );
    }
    let bare_name = command
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !command.starts_with('-')
        && !command.starts_with('.')
        && !command.contains("..");
    if !bare_name {
        return Err(
            "/blacklist only accepts a bare command name, not a glob or a path".to_string(),
        );
    }
    Ok(command)
}

fn bash_blacklist_rules(command: &str) -> [String; 2] {
    [format!("Bash({command})"), format!("Bash({command} *)")]
}

/// Append `Bash(<command>)` and `Bash(<command> *)` to `[permission].deny`.
/// Existing rules and other tables stay. A second write does not duplicate.
pub(crate) fn append_bash_blacklist_at(path: &Path, command: &str) -> Result<[String; 2], String> {
    let command = validate_blacklist_command(command)?;
    let rules = bash_blacklist_rules(command);
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("cannot create config dir: {err}"))?;
    }
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(format!("cannot read config: {err}")),
    };
    let mut doc: toml_edit::DocumentMut = match content.parse() {
        Ok(doc) => doc,
        Err(_) => {
            return Err("config.toml is not valid TOML; refusing to overwrite".to_string());
        }
    };
    match doc.get("permission") {
        None => {
            doc["permission"] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        Some(item) if item.as_table().is_some() => {}
        Some(_) => {
            return Err("[permission] is not a table; refusing to overwrite".to_string());
        }
    }
    let Some(permission) = doc["permission"].as_table_mut() else {
        return Err("[permission] is not a table; refusing to overwrite".to_string());
    };
    match permission.get("deny") {
        None => {
            permission["deny"] =
                toml_edit::Item::Value(toml_edit::Value::Array(toml_edit::Array::new()));
        }
        Some(item) if item.as_array().is_some() => {}
        Some(_) => {
            return Err("permission.deny is not an array; refusing to overwrite".to_string());
        }
    }
    let Some(deny) = permission["deny"].as_array_mut() else {
        return Err("permission.deny is not an array; refusing to overwrite".to_string());
    };
    for rule in &rules {
        let present = deny.iter().any(|item| item.as_str() == Some(rule.as_str()));
        if !present {
            deny.push(rule.as_str());
        }
    }
    let tmp = path.with_extension("blacklist-tmp");
    std::fs::write(&tmp, doc.to_string()).map_err(|err| format!("cannot write config: {err}"))?;
    if let Err(err) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("cannot replace config: {err}"));
    }
    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_workspace::permission::CompiledPolicy;
    use xai_grok_workspace::permission::rules::parse_permission_rule;
    use xai_grok_workspace::permission::types::{
        AccessKind, Decision, PermissionConfig, RuleAction,
    };

    fn deny_strings(body: &str) -> Vec<String> {
        let value: toml::Value = toml::from_str(body).expect("config toml");
        value
            .get("permission")
            .and_then(|p| p.get("deny"))
            .and_then(|d| d.as_array())
            .expect("permission.deny array")
            .iter()
            .map(|item| item.as_str().expect("deny string").to_string())
            .collect()
    }

    fn policy_from_config(body: &str) -> CompiledPolicy {
        let value: toml::Value = toml::from_str(body).expect("config toml");
        let permission = value.get("permission").expect("permission");
        let mut rules = Vec::new();
        for (key, action) in [
            ("deny", RuleAction::Deny),
            ("ask", RuleAction::Ask),
            ("allow", RuleAction::Allow),
        ] {
            let Some(items) = permission.get(key).and_then(|v| v.as_array()) else {
                continue;
            };
            for item in items {
                let text = item.as_str().expect("rule string");
                rules.push(parse_permission_rule(text, action).expect("rule parses"));
            }
        }
        CompiledPolicy::new(PermissionConfig::new(rules))
    }

    fn refused(policy: &CompiledPolicy, cmd: &str) -> bool {
        matches!(
            policy.evaluate_bash_command_policy(cmd),
            Some(Decision::Reject(_)) | Some(Decision::PolicyDeny(_))
        ) || matches!(
            policy.evaluate(&AccessKind::Bash(cmd.to_string())),
            Some(Decision::Reject(_)) | Some(Decision::PolicyDeny(_))
        )
    }

    #[test]
    fn slash_blacklist_lean_records_deny_refuses_lean_or_lean_run_and_not_lake() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[ui]\ntheme = \"dark\"\n\n[permission]\ndeny = [\"Bash(git push)\"]\nallow = [\"Bash(*)\"]\n",
        )
        .unwrap();

        let rules =
            BlacklistCommand::record_at(&path, "lean").expect("/blacklist lean records the deny");
        assert_eq!(
            rules,
            ["Bash(lean)".to_string(), "Bash(lean *)".to_string()]
        );

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("theme"), "other tables stay, got:\n{body}");
        let denied = deny_strings(&body);
        assert!(
            denied.iter().any(|rule| rule == "Bash(git push)"),
            "existing deny stays: {denied:?}"
        );
        assert!(
            denied.iter().any(|rule| rule == "Bash(lean)"),
            "bare lean deny recorded: {denied:?}"
        );
        assert!(
            denied.iter().any(|rule| rule == "Bash(lean *)"),
            "lean with arguments deny recorded: {denied:?}"
        );
        assert_eq!(
            denied
                .iter()
                .filter(|rule| rule.as_str() == "Bash(lean)")
                .count(),
            1
        );

        BlacklistCommand::record_at(&path, "lean").expect("second blacklist is idempotent");
        let denied_again = deny_strings(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(denied, denied_again, "recording twice must not duplicate");

        let policy = policy_from_config(&std::fs::read_to_string(&path).unwrap());
        assert!(refused(&policy, "lean"), "bare lean is refused");
        assert!(refused(&policy, "lean --run"), "lean --run is refused");
        assert!(
            !refused(&policy, "lake"),
            "lake is not refused by the lean rule"
        );
        assert!(
            !refused(&policy, "lake build"),
            "lake with arguments is not refused"
        );
        assert!(
            !refused(&policy, "glean"),
            "glean only contains the letters lean and must not be refused"
        );
        assert!(
            !refused(&policy, "cleaner"),
            "cleaner only contains the letters lean and must not be refused"
        );
        assert!(
            !refused(&policy, "lane"),
            "lane is not the command lean and must not be refused"
        );
    }
}
