//! Auto-run `/implement` follow-ups after a successful turn ends.
//!
//! When enabled (default on), turn end looks for a **sentence/line-leading**
//! `/implement` command and enqueues the **full multi-line block** from the
//! implement token through EOF (body may include later slash lines that are
//! part of residual notes — models often paste a whole next-prompt blob).
//!
//! Sources (in order):
//! 1. **User prompt follow-up** — prior user message has non-implement content
//!    first, then a later `/implement` block (same-message design→implement).
//! 2. **Assistant residual** — last turn’s agent messages contain a trailing
//!    `/implement` block (models should leave “Next implement prompt” near the
//!    end). Skipped when the block is an exact echo of the user prompt just
//!    run (avoids re-queueing the same primary implement).
//!
//! Implement-loop effort may be rewritten via Token Economy: optional lock and
//! min floor always apply when set; when **economic mode** is on and
//! `[token_economy] cap_implement_effort_when_economic` is true, hard ceiling
//! (default 3) and desired inject when missing (default 2) also apply, with a
//! toast on rewrite. See `xai_grok_shell::token_economy`.
//!
//! Enqueue is always **append** (`push_back`): existing local queued prompts
//! are kept; the follow-up `/implement` is added at the end.

use crate::app::agent_view::AgentView;
use crate::scrollback::block::RenderBlock;

/// Toast shown when a follow-up `/implement` is auto-queued after turn end.
pub const AUTO_IMPLEMENT_TOAST: &str = "next task /implement detected, automatically running";

/// Whether `text` is an `/implement` command at the start of the string
/// (optional args after whitespace). Case-insensitive command token.
pub fn is_implement_command_sentence(text: &str) -> bool {
    let t = text.trim_start();
    let lower = t.to_ascii_lowercase();
    if !lower.starts_with("/implement") {
        return false;
    }
    match t.as_bytes().get("/implement".len()) {
        None => true,
        Some(b) => b.is_ascii_whitespace(),
    }
}

/// Split text into sentence-like units (used only to find mid-line implement
/// starts such as `Review the PR. /implement …`).
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            push_unit(&mut out, &text[start..i]);
            start = i + 1;
            i += 1;
            continue;
        }
        if matches!(b, b'.' | b'!' | b'?') {
            let next = i + 1;
            if next >= bytes.len() || bytes[next].is_ascii_whitespace() {
                push_unit(&mut out, &text[start..next]);
                start = next;
                while start < bytes.len()
                    && bytes[start].is_ascii_whitespace()
                    && bytes[start] != b'\n'
                {
                    start += 1;
                }
                i = start;
                continue;
            }
        }
        i += 1;
    }
    push_unit(&mut out, &text[start..]);
    out
}

fn push_unit(out: &mut Vec<String>, s: &str) {
    let t = s.trim();
    if !t.is_empty() {
        out.push(t.to_string());
    }
}

/// From byte offset `start` (must point at `/implement…`), take the full block
/// through EOF (trimmed). Body is not cut at a later slash command — residual
/// prompts often include notes or nested paths after the implement line.
pub fn extract_implement_block_at(text: &str, start: usize) -> Option<String> {
    if start >= text.len() || !is_implement_command_sentence(&text[start..]) {
        return None;
    }
    let trimmed = text[start..].trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Apply Token Economy implement-effort policy when economic mode + cap master
/// are on. Returns the (possibly rewritten) command string.
///
/// Prefer [`apply_implement_effort_for_product`] when the toast is needed.
pub fn clamp_implement_effort_for_economic_mode(cmd: &str, economic_mode: bool) -> String {
    apply_implement_effort_for_product(cmd, economic_mode).command
}

/// Full product rewrite: command + optional toast (clamp or desired inject).
pub fn apply_implement_effort_for_product(
    cmd: &str,
    economic_mode: bool,
) -> xai_grok_shell::token_economy::ImplementEffortRewrite {
    let cfg = xai_grok_shell::token_economy::token_economy_from_disk();
    xai_grok_shell::token_economy::apply_implement_effort_policy(cmd, economic_mode, &cfg)
}

/// Same as [`apply_implement_effort_for_product`] with an explicit config
/// (unit tests / settings preview).
pub fn apply_implement_effort_with_config(
    cmd: &str,
    economic_mode: bool,
    cfg: &xai_grok_shell::token_economy::TokenEconomyConfig,
) -> xai_grok_shell::token_economy::ImplementEffortRewrite {
    xai_grok_shell::token_economy::apply_implement_effort_policy(cmd, economic_mode, cfg)
}

/// Byte offset of a follow-up implement start in `text`, or `None` when the
/// prompt’s primary content already is implement / no implement exists.
fn find_followup_implement_offset(text: &str) -> Option<usize> {
    let mut saw_non_implement = false;
    let mut pos = 0usize;
    while pos <= text.len() {
        let nl = text[pos..].find('\n').map(|i| pos + i);
        let end = nl.unwrap_or(text.len());
        let line = &text[pos..end];

        if !line.trim().is_empty() {
            let trim_off = line.len() - line.trim_start().len();
            let body = &line[trim_off..];
            if is_implement_command_sentence(body) {
                if saw_non_implement {
                    return Some(pos + trim_off);
                }
                // Primary turn is implement — do not auto from user prompt.
                return None;
            }
            // Mid-line: "Review the PR. /implement …"
            for unit in split_sentences(line) {
                if is_implement_command_sentence(&unit)
                    && let Some(rel) = find_implement_token_offset(line)
                {
                    let before = line[..rel].trim();
                    if !before.is_empty() || saw_non_implement {
                        return Some(pos + rel);
                    }
                }
            }
            saw_non_implement = true;
        }

        if nl.is_none() {
            break;
        }
        pos = end + 1;
    }
    None
}

/// Case-insensitive index of `/implement` as a command token on `line`.
fn find_implement_token_offset(line: &str) -> Option<usize> {
    let lower = line.to_ascii_lowercase();
    let mut search_from = 0usize;
    while let Some(rel) = lower[search_from..].find("/implement") {
        let abs = search_from + rel;
        let after = abs + "/implement".len();
        let boundary_ok = match line.as_bytes().get(after) {
            None => true,
            Some(b) => b.is_ascii_whitespace(),
        };
        if boundary_ok {
            // Sentence-leading: start of line or after `.!?` + whitespace.
            let prefix = line[..abs].trim_end();
            if prefix.is_empty()
                || prefix.ends_with('.')
                || prefix.ends_with('!')
                || prefix.ends_with('?')
            {
                return Some(abs);
            }
        }
        search_from = abs + 1;
    }
    None
}

/// Extract a follow-up multi-line `/implement` block from the prior user prompt.
///
/// Returns `None` when no follow-up exists or the primary turn is already
/// implement (first non-empty line starts with `/implement`).
pub fn extract_auto_implement_followup(prior_prompt: &str) -> Option<String> {
    let start = find_followup_implement_offset(prior_prompt)?;
    extract_implement_block_at(prior_prompt, start)
}

/// Extract the **last** full multi-line `/implement` block from `text`
/// (prefer residual “next implement” near the end of a report).
pub fn extract_last_implement_block(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let mut last: Option<usize> = None;
    let mut search_from = 0usize;
    while let Some(rel) = lower[search_from..].find("/implement") {
        let abs = search_from + rel;
        let after = abs + "/implement".len();
        let boundary_ok = match text.as_bytes().get(after) {
            None => true,
            Some(b) => b.is_ascii_whitespace(),
        };
        if boundary_ok {
            // Line-leading only for residual assistant blocks (avoid mid-prose).
            let line_start = text[..abs].rfind('\n').map(|i| i + 1).unwrap_or(0);
            if text[line_start..abs].trim().is_empty() {
                last = Some(abs);
            }
        }
        search_from = abs + 1;
    }
    last.and_then(|s| extract_implement_block_at(text, s))
}

/// Collect agent-message source markdown from the most recent turn
/// (everything after the last user prompt in scrollback).
pub fn last_turn_assistant_text(agent: &AgentView) -> Option<String> {
    let len = agent.scrollback.len();
    if len == 0 {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    for i in (0..len).rev() {
        let Some(entry) = agent.scrollback.entry(i) else {
            continue;
        };
        match &entry.block {
            RenderBlock::UserPrompt(_) => break,
            RenderBlock::AgentMessage(m) => {
                let t = m.text();
                if !t.trim().is_empty() {
                    parts.push(t);
                }
            }
            _ => {}
        }
    }
    if parts.is_empty() {
        return None;
    }
    parts.reverse();
    Some(parts.join("\n"))
}

/// After a successful non-cancel agent turn, maybe queue a multi-line
/// `/implement` block. Returns `Some(toast)` when enqueued (caller should
/// show toast + drain), or `None` when nothing was queued.
pub fn maybe_enqueue_auto_implement(agent: &mut AgentView, enabled: bool) -> Option<String> {
    if !enabled {
        return None;
    }
    if agent.attached_as_viewer {
        return None;
    }
    if agent.bash_turn {
        return None;
    }
    // Always append to the end of the local queue (enqueue_prompt is push_back).
    // Do not drop or skip when other prompts are already waiting — keep them
    // and add /implement after. Shared/server queue rows are separate; local
    // append still records the follow-up for when the session is free to drain.

    let prior = agent.session.prompt_history.first().cloned();

    // 1) Follow-up implement in the same user message (design then /implement …).
    let from_user = prior.as_deref().and_then(extract_auto_implement_followup);

    // 2) Trailing residual block in the assistant’s just-finished turn.
    let from_assistant = last_turn_assistant_text(agent)
        .as_deref()
        .and_then(extract_last_implement_block)
        .filter(|cmd| {
            // Don't re-queue an exact echo of the prompt that just ran.
            prior
                .as_deref()
                .map(|p| p.trim() != cmd.trim())
                .unwrap_or(true)
        });

    let raw = from_user.or(from_assistant)?;

    let economic = crate::appearance::cache::load_economic_mode();
    let rewrite = apply_implement_effort_for_product(&raw, economic);
    let cmd = rewrite.command;
    let toast = auto_implement_toast_for(&raw, &cmd, economic, rewrite.toast.as_deref());

    let ranges = agent
        .prompt
        .slash_controller
        .recognized_token_ranges(&cmd, &agent.session.models);
    agent.session.enqueue_prompt_with_skill_tokens(cmd, ranges);
    Some(toast)
}

/// Toast when a follow-up was auto-queued.
///
/// When Token Economy rewrote implement effort, prefer that toast; otherwise
/// the plain auto-run copy. `effort_toast` is the optional rewrite message.
pub fn auto_implement_toast_for(
    _raw_cmd: &str,
    _enqueued_cmd: &str,
    _economic_mode: bool,
    effort_toast: Option<&str>,
) -> String {
    if let Some(t) = effort_toast.filter(|s| !s.is_empty()) {
        // Combine so operators still know auto-run fired.
        format!("{AUTO_IMPLEMENT_TOAST} · {t}")
    } else {
        AUTO_IMPLEMENT_TOAST.to_string()
    }
}

/// After a clean agent turn ends (before queue drain): enqueue a follow-up
/// `/implement` when the setting is on, and toast.
///
/// Call only on successful, non-cancel, non-bash turn ends.
pub fn on_successful_turn_end(agent: &mut AgentView) {
    let enabled = crate::appearance::cache::load_auto_run_implement();
    if let Some(toast) = maybe_enqueue_auto_implement(agent, enabled) {
        agent.show_toast(&toast);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_implement_accepts_bare_and_args() {
        assert!(is_implement_command_sentence("/implement"));
        assert!(is_implement_command_sentence("  /implement fix tests"));
        assert!(is_implement_command_sentence("/IMPLEMENT the plan"));
        assert!(!is_implement_command_sentence("/implements"));
        assert!(!is_implement_command_sentence("please /implement later"));
        assert!(!is_implement_command_sentence("/goal do stuff"));
    }

    #[test]
    fn extract_skips_when_primary_is_implement() {
        assert_eq!(
            extract_auto_implement_followup("/implement fix the gate"),
            None
        );
        assert_eq!(
            extract_auto_implement_followup("/implement\n/implement again"),
            None
        );
        assert_eq!(
            extract_auto_implement_followup(
                "/implement --effort 5 residual work:\n1) wire Systems.Proc\n2) keep SCORE fail=0"
            ),
            None
        );
    }

    #[test]
    fn extract_finds_followup_after_plain_sentence() {
        assert_eq!(
            extract_auto_implement_followup(
                "Design a hermetic test plan.\n/implement the plan carefully"
            )
            .as_deref(),
            Some("/implement the plan carefully")
        );
        assert_eq!(
            extract_auto_implement_followup(
                "Review the PR. /implement any remaining test failures"
            )
            .as_deref(),
            Some("/implement any remaining test failures")
        );
    }

    #[test]
    fn extract_grabs_full_multiline_implement_block() {
        let prior = "\
Highest-value residual next (bottleneck order)
1. Slake_Proc_dogfood — wire freestanding Systems.Proc

---

Next implement prompt

/implement --effort 5 all remaining planned residual in priority order:
1) Slake_Proc_dogfood — freestanding Systems.Proc (or Extract C ABI)
2) TomlConfig_more — expand TomlConfigLite toward more lakefile.toml keys
3) Track L residual-green product trio if parallelizable
4) Do NOT claim H5 residual_free elaborator unless measured GC_FREE_ELABORATOR=1
Update RESIDUAL.md PRODUCT_FS_NEXT. Stay in cwd; subagent hierarchy; SCORE fail=0.";

        let got = extract_auto_implement_followup(prior).expect("follow-up block");
        assert!(
            got.starts_with("/implement --effort 5 all remaining planned residual"),
            "must start with implement line: {got}"
        );
        assert!(
            got.contains("1) Slake_Proc_dogfood"),
            "must include body line 1: {got}"
        );
        assert!(
            got.contains("4) Do NOT claim H5"),
            "must include body line 4: {got}"
        );
        assert!(
            got.contains("Update RESIDUAL.md PRODUCT_FS_NEXT"),
            "must include trailing body: {got}"
        );
        assert!(
            got.lines().count() >= 5,
            "expected multi-line block, got {} lines: {got}",
            got.lines().count()
        );
    }

    #[test]
    fn extract_grabs_through_eof_including_later_slash_lines() {
        // Residual blobs sometimes include other slash notes after the body;
        // take everything through EOF so we do not truncate mid-prompt.
        let prior = "\
Plan first.

/implement do the wiring
1) keep dual residual honest
/review check security after
more review notes";
        let got = extract_auto_implement_followup(prior).expect("block");
        assert!(got.contains("1) keep dual residual honest"));
        assert!(
            got.contains("/review check security after"),
            "must keep later slash lines through EOF: {got}"
        );
        assert!(got.contains("more review notes"));
    }

    /// Named contract (Token Economy): economic on + default max 3 clamps
    /// effort 5 → 3; effort 2 stays 2; missing injects desired 2; off = identity.
    #[test]
    fn economic_implement_effort_policy_ceiling_desired_and_off() {
        let cfg = xai_grok_shell::token_economy::TokenEconomyConfig::default();

        // Effort 2 stays 2 (not the old silent clamp-to-1).
        let effort2 = "/implement --effort 2 Linear freestanding contracts";
        let r2 = apply_implement_effort_with_config(effort2, true, &cfg);
        assert_eq!(r2.command, effort2);
        assert!(r2.toast.is_none());

        // Over ceiling → max + toast.
        let high = "/implement --effort 5 residual work:\n1) wire Systems.Proc";
        let r5 = apply_implement_effort_with_config(high, true, &cfg);
        assert!(
            r5.command.starts_with("/implement --effort 3 "),
            "effort 5 must clamp to max 3: {}",
            r5.command
        );
        assert!(r5.command.contains("1) wire Systems.Proc"));
        assert!(
            r5.toast
                .as_deref()
                .is_some_and(|t| t.contains("capped at 3") && t.contains("was 5")),
            "{:?}",
            r5.toast
        );

        // Economic off → no rewrite.
        assert_eq!(
            apply_implement_effort_with_config(high, false, &cfg).command,
            high
        );

        // Missing → desired 2.
        let none = "/implement fix the gate";
        let r_none = apply_implement_effort_with_config(none, true, &cfg);
        assert_eq!(r_none.command, "/implement --effort 2 fix the gate");
        assert!(r_none.toast.is_some());

        // clamp helper matches command field.
        assert_eq!(clamp_implement_effort_for_economic_mode(high, false), high);
    }

    /// Inventory: product entry paths that must apply the shared helper.
    #[test]
    fn implement_effort_entry_paths_use_shared_helper() {
        // Call-site inventory (keep in sync when adding paths):
        // 1) auto_implement::maybe_enqueue_auto_implement
        // 2) dispatch_send_prompt_inner PassThrough / plain implement submit
        // Both call apply_implement_effort_for_product / clamp_*.
        let src = include_str!("auto_implement.rs");
        assert!(
            src.contains("apply_implement_effort_for_product"),
            "auto_implement must call shared product helper"
        );
        let prompt_src = include_str!("dispatch/prompt.rs");
        assert!(
            prompt_src.contains("apply_implement_effort_for_product")
                || prompt_src.contains("clamp_implement_effort_for_economic_mode"),
            "dispatch_send_prompt_inner must apply implement effort policy"
        );
    }

    #[test]
    fn last_implement_block_prefers_trailing_residual() {
        let assistant = "\
## Summary
Done with score green.

Early note:
/implement never_use_this
1) old body

## Next implement prompt
/implement --effort 5 remaining residual:
1) Slake_Proc_dogfood
2) TomlConfig_more
Stay in cwd.";
        let got = extract_last_implement_block(assistant).expect("last block");
        assert!(got.contains("Slake_Proc_dogfood"));
        assert!(
            !got.contains("never_use_this"),
            "must prefer last block: {got}"
        );
        assert!(got.lines().count() >= 3);
    }

    #[test]
    fn path_like_slash_does_not_end_block() {
        let prior = "\
Do the work.

/implement fix crates/codegen/xai-foo/src/lib.rs
1) edit the file
2) run tests";
        let got = extract_auto_implement_followup(prior).expect("block");
        assert!(got.contains("1) edit the file"));
        assert!(got.contains("2) run tests"));
    }

    #[test]
    fn extract_none_without_implement() {
        assert_eq!(
            extract_auto_implement_followup("just review the code please"),
            None
        );
    }

    #[test]
    fn toast_copy_matches_product() {
        assert_eq!(
            AUTO_IMPLEMENT_TOAST,
            "next task /implement detected, automatically running"
        );
        let cfg = xai_grok_shell::token_economy::TokenEconomyConfig::default();
        let raw = "/implement --effort 5 residual";
        let rewrite = apply_implement_effort_with_config(raw, true, &cfg);
        assert!(rewrite.command.starts_with("/implement --effort 3 "));
        let toast = auto_implement_toast_for(raw, &rewrite.command, true, rewrite.toast.as_deref());
        assert!(toast.contains(AUTO_IMPLEMENT_TOAST));
        assert!(toast.contains("capped at 3"));
        // No rewrite → plain auto-run toast.
        assert_eq!(
            auto_implement_toast_for(raw, raw, false, None),
            AUTO_IMPLEMENT_TOAST
        );
    }
}
