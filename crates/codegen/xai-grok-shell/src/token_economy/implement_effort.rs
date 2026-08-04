//! Implement-loop effort (1–5 reviewer fan-out) under Token Economy.
//!
//! This is **not** model reasoning effort (`low` / `high`).
//!
//! Application order (documented in user-guide Token Economy):
//! 1. If `lock_implement_effort` is set → start from that value (ignore prompt
//!    effort and desired inject).
//! 2. Else: missing effort → inject `desired_implement_effort` when economic
//!    caps are active; present effort stays as written.
//! 3. Apply floor `min_implement_effort` (always; raise if below). When effort
//!    is still missing and min > 1, inject the floor.
//! 4. If economic mode + `cap_implement_effort_when_economic`: apply ceiling
//!    `max_implement_effort` (lower if above).
//!
//! Min and lock are **not** economic-only. Ceiling and desired inject still
//! require economic mode + the cap master.

use super::config::{TokenEconomyConfig, implement_effort_policy_active};

/// Result of applying implement-effort policy to a command string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementEffortRewrite {
    /// Command after policy (may be unchanged).
    pub command: String,
    /// Toast when effort was rewritten (lock, floor, desired inject, or ceiling).
    pub toast: Option<String>,
    /// Effort present in the original command before rewrite.
    pub original_effort: Option<u8>,
    /// Final effort after policy (always set when an implement cmd was rewritten
    /// or already had a parseable effort / injected value).
    pub final_effort: Option<u8>,
}

/// Apply Token Economy implement-effort policy to an `/implement` command line
/// (or multi-line block starting with `/implement`).
///
/// Non-implement text is returned unchanged. Min floor and lock always apply
/// when they would change the effort. Economic ceiling + desired inject apply
/// only when economic mode is on and the cap master is true.
pub fn apply_implement_effort_policy(
    cmd: &str,
    economic_mode: bool,
    cfg: &TokenEconomyConfig,
) -> ImplementEffortRewrite {
    if !is_implement_command(cmd) {
        return ImplementEffortRewrite {
            command: cmd.to_string(),
            toast: None,
            original_effort: None,
            final_effort: None,
        };
    }

    let original_effort = parse_implement_effort(cmd);
    let economic_caps = implement_effort_policy_active(economic_mode, cfg);
    let min = cfg.min_implement_effort;
    let max = cfg.max_implement_effort;
    let desired = cfg.desired_implement_effort.min(max);

    // Track which rule produced the last meaningful rewrite for toast copy.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Rule {
        Lock,
        Desired,
        MinFloor,
        Ceiling,
    }

    // Step 1–2: base effort.
    let mut working: Option<u8>;
    let mut rule: Option<Rule>;
    if let Some(lock) = cfg.lock_implement_effort {
        working = Some(lock);
        rule = Some(Rule::Lock);
    } else if let Some(n) = original_effort {
        working = Some(n);
        rule = None;
    } else if economic_caps {
        working = Some(desired);
        rule = Some(Rule::Desired);
    } else {
        working = None;
        rule = None;
    }

    // Step 3: floor (always). Missing + min > 1 → inject floor.
    match working {
        Some(w) if w < min => {
            working = Some(min);
            rule = Some(Rule::MinFloor);
        }
        None if min > 1 => {
            working = Some(min);
            rule = Some(Rule::MinFloor);
        }
        _ => {}
    }

    // Step 4: economic ceiling.
    if economic_caps {
        if let Some(w) = working {
            if w > max {
                working = Some(max);
                rule = Some(Rule::Ceiling);
            }
        }
    }

    let final_effort = working;

    // No target effort → leave command alone (defaults: min 1, no lock, economic off).
    let Some(final_n) = final_effort else {
        return ImplementEffortRewrite {
            command: cmd.to_string(),
            toast: None,
            original_effort,
            final_effort: original_effort,
        };
    };

    // Unchanged value already present in the command.
    if original_effort == Some(final_n) {
        return ImplementEffortRewrite {
            command: cmd.to_string(),
            toast: None,
            original_effort,
            final_effort: Some(final_n),
        };
    }

    // Rewrite: inject if missing, else replace the parsed value.
    let rewritten = match original_effort {
        None => inject_effort(cmd, final_n),
        Some(from) => replace_effort(cmd, from, final_n),
    };

    let toast = match rule {
        Some(Rule::Lock) => Some(match original_effort {
            Some(was) => {
                format!("Implement effort locked at {final_n} (was {was}; lock_implement_effort).")
            }
            None => format!("Implement effort locked at {final_n} (lock_implement_effort)."),
        }),
        Some(Rule::Desired) => Some(format!(
            "Economic mode: implement effort set to {final_n} (desired under economic mode)."
        )),
        Some(Rule::MinFloor) => Some(match original_effort {
            Some(was) => {
                format!("Implement effort raised to {final_n} (was {was}; min_implement_effort).")
            }
            None => format!("Implement effort set to {final_n} (min_implement_effort)."),
        }),
        Some(Rule::Ceiling) => {
            // Prefer the prompt's original value when present (human-visible "was").
            let was = original_effort.unwrap_or(final_n);
            Some(format!(
                "Economic mode: implement effort capped at {final_n} (was {was})."
            ))
        }
        None => {
            // Should not happen when original != final, but keep a safe toast.
            Some(format!("Implement effort set to {final_n}."))
        }
    };

    ImplementEffortRewrite {
        command: rewritten,
        toast,
        original_effort,
        final_effort: Some(final_n),
    }
}

/// True when `text` is an `/implement` command (optional leading whitespace).
pub fn is_implement_command(text: &str) -> bool {
    let t = text.trim_start();
    let lower = t.to_ascii_lowercase();
    if !lower.starts_with("/implement") {
        return false;
    }
    match t.as_bytes().get("/implement".len()) {
        None => true,
        Some(b) => b.is_ascii_whitespace() || *b == b'\n',
    }
}

/// Parse `--effort N` or bare `effort N` from the first line of an implement cmd.
pub fn parse_implement_effort(cmd: &str) -> Option<u8> {
    let first = cmd.lines().next().unwrap_or(cmd);
    let lower = first.to_ascii_lowercase();
    // Prefer --effort N
    if let Some(n) = parse_flag_effort(&lower, "--effort") {
        return Some(n);
    }
    // Bare "effort N" after /implement (not mid-word)
    parse_flag_effort(&lower, "effort")
}

fn parse_flag_effort(lower_line: &str, flag: &str) -> Option<u8> {
    let mut search = 0usize;
    while let Some(rel) = lower_line[search..].find(flag) {
        let abs = search + rel;
        // Boundary before flag
        if abs > 0 {
            let prev = lower_line.as_bytes()[abs - 1];
            if !prev.is_ascii_whitespace() {
                search = abs + 1;
                continue;
            }
        }
        let after = abs + flag.len();
        let rest = lower_line.get(after..)?;
        let rest = rest.trim_start();
        // Optional `=` then digits
        let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            search = abs + 1;
            continue;
        }
        // Must be end or whitespace after number
        let after_num = rest.get(digits.len()..).unwrap_or("");
        if !after_num.is_empty() {
            let b = after_num.as_bytes()[0];
            if !b.is_ascii_whitespace() {
                search = abs + 1;
                continue;
            }
        }
        let n: u8 = digits.parse().ok()?;
        if (1..=5).contains(&n) {
            return Some(n);
        }
        return None;
    }
    None
}

/// Insert `--effort N` after `/implement` when missing.
fn inject_effort(cmd: &str, effort: u8) -> String {
    let trim_start_len = cmd.len() - cmd.trim_start().len();
    let head = &cmd[..trim_start_len];
    let body = &cmd[trim_start_len..];
    // Case-preserving: keep original "/implement" spelling length.
    let token_len = "/implement".len();
    if body.len() < token_len {
        return format!("{head}/implement --effort {effort}");
    }
    let after_token = &body[token_len..];
    // Preserve original token casing
    let token = &body[..token_len];
    if after_token.is_empty() {
        return format!("{head}{token} --effort {effort}");
    }
    // after_token starts with whitespace or newline
    format!("{head}{token} --effort {effort}{after_token}")
}

/// Replace an existing effort value `from` with `to` on the first line.
fn replace_effort(cmd: &str, from: u8, to: u8) -> String {
    let mut lines = cmd.lines();
    let Some(first) = lines.next() else {
        return cmd.to_string();
    };
    let from_s = from.to_string();
    let to_s = to.to_string();
    let lower = first.to_ascii_lowercase();

    // Prefer replacing after --effort
    let rewritten_first = if let Some(pos) = find_effort_value_span(&lower, "--effort", &from_s) {
        replace_span(first, pos, &to_s)
    } else if let Some(pos) = find_effort_value_span(&lower, "effort", &from_s) {
        replace_span(first, pos, &to_s)
    } else {
        // Fallback: inject max and leave old (should not happen if parse worked)
        return inject_effort(cmd, to);
    };

    let rest: Vec<&str> = lines.collect();
    if rest.is_empty() {
        rewritten_first
    } else {
        format!("{rewritten_first}\n{}", rest.join("\n"))
    }
}

fn find_effort_value_span(lower_line: &str, flag: &str, value: &str) -> Option<(usize, usize)> {
    let mut search = 0usize;
    while let Some(rel) = lower_line[search..].find(flag) {
        let abs = search + rel;
        if abs > 0 && !lower_line.as_bytes()[abs - 1].is_ascii_whitespace() {
            search = abs + 1;
            continue;
        }
        let after = abs + flag.len();
        let rest = &lower_line[after..];
        let trimmed_ws = rest.len() - rest.trim_start().len();
        let mut rest2 = rest.trim_start();
        let mut eq_extra = 0usize;
        if let Some(r) = rest2.strip_prefix('=') {
            eq_extra = 1 + (r.len() - r.trim_start().len());
            rest2 = r.trim_start();
        }
        if rest2.starts_with(value) {
            let end_ok = match rest2.as_bytes().get(value.len()) {
                None => true,
                Some(b) => b.is_ascii_whitespace(),
            };
            if end_ok {
                let start = after + trimmed_ws + eq_extra;
                return Some((start, start + value.len()));
            }
        }
        search = abs + 1;
    }
    None
}

fn replace_span(s: &str, (start, end): (usize, usize), with: &str) -> String {
    format!("{}{}{}", &s[..start], with, &s[end..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_economy::config::TokenEconomyConfig;

    fn on_cfg() -> TokenEconomyConfig {
        TokenEconomyConfig::default()
    }

    #[test]
    fn economic_off_leaves_effort_5() {
        let raw = "/implement --effort 5 residual work";
        let got = apply_implement_effort_policy(raw, false, &on_cfg());
        assert_eq!(got.command, raw);
        assert!(got.toast.is_none());
        assert_eq!(got.original_effort, Some(5));
        assert_eq!(got.final_effort, Some(5));
    }

    #[test]
    fn cap_master_false_leaves_effort_5() {
        let mut cfg = on_cfg();
        cfg.cap_implement_effort_when_economic = false;
        let raw = "/implement --effort 5 residual";
        let got = apply_implement_effort_policy(raw, true, &cfg);
        assert_eq!(got.command, raw);
        assert!(got.toast.is_none());
    }

    #[test]
    fn economic_on_clamps_effort_5_to_max_3_with_toast() {
        let raw = "/implement --effort 5 residual work:\n1) wire Systems.Proc";
        let got = apply_implement_effort_policy(raw, true, &on_cfg());
        assert!(
            got.command.starts_with("/implement --effort 3 "),
            "got: {}",
            got.command
        );
        assert!(got.command.contains("1) wire Systems.Proc"));
        assert_eq!(
            got.toast.as_deref(),
            Some("Economic mode: implement effort capped at 3 (was 5).")
        );
        assert_eq!(got.original_effort, Some(5));
        assert_eq!(got.final_effort, Some(3));
    }

    #[test]
    fn effort_2_stays_2_under_default_ceiling() {
        let raw = "/implement --effort 2 Linear freestanding contracts";
        let got = apply_implement_effort_policy(raw, true, &on_cfg());
        assert_eq!(got.command, raw);
        assert!(got.toast.is_none());
        assert_eq!(got.final_effort, Some(2));
    }

    #[test]
    fn missing_effort_injects_desired_2() {
        let raw = "/implement fix the gate";
        let got = apply_implement_effort_policy(raw, true, &on_cfg());
        assert_eq!(got.command, "/implement --effort 2 fix the gate");
        assert!(
            got.toast.as_deref().is_some_and(|t| t.contains("set to 2")),
            "{:?}",
            got.toast
        );
        assert_eq!(got.final_effort, Some(2));
    }

    #[test]
    fn bare_effort_form_clamped() {
        let raw = "/implement effort 5 do the thing";
        let got = apply_implement_effort_policy(raw, true, &on_cfg());
        assert!(
            got.command.contains("effort 3") || got.command.contains("--effort 3"),
            "{}",
            got.command
        );
        assert_eq!(got.original_effort, Some(5));
        assert_eq!(got.final_effort, Some(3));
    }

    #[test]
    fn non_implement_unchanged() {
        let raw = "please review the PR";
        let got = apply_implement_effort_policy(raw, true, &on_cfg());
        assert_eq!(got.command, raw);
        assert!(got.toast.is_none());
    }

    #[test]
    fn custom_max_and_desired() {
        let mut cfg = on_cfg();
        cfg.max_implement_effort = 4;
        cfg.desired_implement_effort = 3;
        let miss = apply_implement_effort_policy("/implement ship it", true, &cfg);
        assert_eq!(miss.command, "/implement --effort 3 ship it");
        let high = apply_implement_effort_policy("/implement --effort 5 ship", true, &cfg);
        assert!(high.command.starts_with("/implement --effort 4 "));
        assert_eq!(high.final_effort, Some(4));
    }

    #[test]
    fn multiline_body_preserved_on_clamp() {
        let raw = "/implement --effort 5 all remaining:\n1) a\n2) b\nUpdate RESIDUAL.md";
        let got = apply_implement_effort_policy(raw, true, &on_cfg());
        assert!(got.command.contains("1) a"));
        assert!(got.command.contains("Update RESIDUAL.md"));
        assert!(got.command.starts_with("/implement --effort 3 "));
    }

    // --- min_implement_effort / lock_implement_effort ---

    #[test]
    fn min_2_raises_effort_1_with_toast() {
        let mut cfg = on_cfg();
        cfg.min_implement_effort = 2;
        let raw = "/implement --effort 1 residual";
        let got = apply_implement_effort_policy(raw, true, &cfg);
        assert!(
            got.command.starts_with("/implement --effort 2 "),
            "{}",
            got.command
        );
        assert_eq!(got.final_effort, Some(2));
        assert!(
            got.toast
                .as_deref()
                .is_some_and(|t| t.contains("raised to 2") && t.contains("was 1")),
            "{:?}",
            got.toast
        );
    }

    #[test]
    fn min_2_effort_3_stays_3_under_economic_max() {
        let mut cfg = on_cfg();
        cfg.min_implement_effort = 2;
        // default max 3
        let raw = "/implement --effort 3 residual";
        let got = apply_implement_effort_policy(raw, true, &cfg);
        assert_eq!(got.command, raw);
        assert!(got.toast.is_none());
        assert_eq!(got.final_effort, Some(3));
    }

    #[test]
    fn lock_2_forces_prompt_effort_5_to_2() {
        let mut cfg = on_cfg();
        cfg.lock_implement_effort = Some(2);
        cfg.min_implement_effort = 1;
        let raw = "/implement --effort 5 residual";
        // economic on: lock 2 ≤ max 3 → final 2
        let got = apply_implement_effort_policy(raw, true, &cfg);
        assert!(
            got.command.starts_with("/implement --effort 2 "),
            "{}",
            got.command
        );
        assert_eq!(got.final_effort, Some(2));
        assert!(
            got.toast
                .as_deref()
                .is_some_and(|t| t.contains("locked at 2") && t.contains("was 5")),
            "{:?}",
            got.toast
        );
    }

    #[test]
    fn lock_2_missing_effort_injects_2_not_desired() {
        let mut cfg = on_cfg();
        cfg.lock_implement_effort = Some(2);
        cfg.desired_implement_effort = 3;
        cfg.max_implement_effort = 5;
        let raw = "/implement fix the gate";
        let got = apply_implement_effort_policy(raw, true, &cfg);
        assert_eq!(got.command, "/implement --effort 2 fix the gate");
        assert_eq!(got.final_effort, Some(2));
        assert!(
            got.toast
                .as_deref()
                .is_some_and(|t| t.contains("locked at 2")),
            "{:?}",
            got.toast
        );
        assert!(
            !got.toast.as_deref().is_some_and(|t| t.contains("desired")),
            "must not use desired toast: {:?}",
            got.toast
        );
    }

    #[test]
    fn economic_off_min_2_still_floors() {
        let mut cfg = on_cfg();
        cfg.min_implement_effort = 2;
        let raw = "/implement --effort 1 residual";
        let got = apply_implement_effort_policy(raw, false, &cfg);
        assert!(
            got.command.starts_with("/implement --effort 2 "),
            "{}",
            got.command
        );
        assert_eq!(got.final_effort, Some(2));
        assert!(
            got.toast
                .as_deref()
                .is_some_and(|t| t.contains("raised to 2")),
            "{:?}",
            got.toast
        );
    }

    #[test]
    fn economic_off_lock_2_still_locks() {
        let mut cfg = on_cfg();
        cfg.lock_implement_effort = Some(2);
        let raw = "/implement --effort 5 residual";
        let got = apply_implement_effort_policy(raw, false, &cfg);
        assert!(
            got.command.starts_with("/implement --effort 2 "),
            "{}",
            got.command
        );
        assert_eq!(got.final_effort, Some(2));
        assert!(
            got.toast
                .as_deref()
                .is_some_and(|t| t.contains("locked at 2"))
        );
    }

    #[test]
    fn economic_on_max_3_min_2_prompt_5_becomes_3() {
        let mut cfg = on_cfg();
        cfg.min_implement_effort = 2;
        cfg.max_implement_effort = 3;
        let raw = "/implement --effort 5 residual";
        let got = apply_implement_effort_policy(raw, true, &cfg);
        assert!(
            got.command.starts_with("/implement --effort 3 "),
            "{}",
            got.command
        );
        assert_eq!(got.final_effort, Some(3));
        assert!(
            got.toast
                .as_deref()
                .is_some_and(|t| t.contains("capped at 3") && t.contains("was 5")),
            "{:?}",
            got.toast
        );
    }

    #[test]
    fn economic_on_max_3_min_2_prompt_1_becomes_2() {
        let mut cfg = on_cfg();
        cfg.min_implement_effort = 2;
        cfg.max_implement_effort = 3;
        let raw = "/implement --effort 1 residual";
        let got = apply_implement_effort_policy(raw, true, &cfg);
        assert!(
            got.command.starts_with("/implement --effort 2 "),
            "{}",
            got.command
        );
        assert_eq!(got.final_effort, Some(2));
        assert!(
            got.toast
                .as_deref()
                .is_some_and(|t| t.contains("raised to 2") && t.contains("was 1")),
            "{:?}",
            got.toast
        );
    }

    #[test]
    fn economic_off_defaults_leave_missing_effort_alone() {
        // Default min=1, no lock → no rewrite when economic caps are off.
        let raw = "/implement fix the gate";
        let got = apply_implement_effort_policy(raw, false, &on_cfg());
        assert_eq!(got.command, raw);
        assert!(got.toast.is_none());
        assert_eq!(got.final_effort, None);
    }

    #[test]
    fn economic_off_min_2_injects_when_missing() {
        let mut cfg = on_cfg();
        cfg.min_implement_effort = 2;
        let raw = "/implement fix the gate";
        let got = apply_implement_effort_policy(raw, false, &cfg);
        assert_eq!(got.command, "/implement --effort 2 fix the gate");
        assert_eq!(got.final_effort, Some(2));
        assert!(got.toast.is_some());
    }

    #[test]
    fn lock_then_runtime_ceiling_if_max_lowered() {
        // Config validation requires lock ≤ max, but if max is lowered mid-session
        // without re-validation, ceiling still applies under economic caps.
        let mut cfg = on_cfg();
        cfg.lock_implement_effort = Some(3);
        cfg.max_implement_effort = 2; // invalid if loaded from toml; runtime still clamps
        cfg.min_implement_effort = 1;
        let raw = "/implement --effort 5 residual";
        let got = apply_implement_effort_policy(raw, true, &cfg);
        assert!(
            got.command.starts_with("/implement --effort 2 "),
            "{}",
            got.command
        );
        assert_eq!(got.final_effort, Some(2));
        assert!(
            got.toast
                .as_deref()
                .is_some_and(|t| t.contains("capped at 2")),
            "{:?}",
            got.toast
        );
    }
}
