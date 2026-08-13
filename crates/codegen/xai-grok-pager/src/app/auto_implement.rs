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
//! Explicit `/implement --effort N` is **honored** on auto-queue (economic mode
//! only soft-caps context; it does not rewrite the operator’s effort flag).
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

/// Preserve the implement command text for auto-queue.
///
/// Previously this rewrote `--effort N` / `effort N` above 1 down to 1 when
/// economic mode was on. That silently ignored an explicit operator request
/// (e.g. `/implement --effort 2 …` became effort 1). Economic mode still
/// soft-caps context; it must not revise implement effort. `economic_mode` is
/// retained so call sites stay stable.
pub fn clamp_implement_effort_for_economic_mode(cmd: &str, _economic_mode: bool) -> String {
    cmd.to_string()
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
    let cmd = clamp_implement_effort_for_economic_mode(&raw, economic);
    let toast = auto_implement_toast_for(&raw, &cmd, economic);

    let ranges = agent
        .prompt
        .slash_controller
        .recognized_token_ranges(&cmd, &agent.session.models);
    agent.session.enqueue_prompt_with_skill_tokens(cmd, ranges);
    Some(toast)
}

/// Toast when a follow-up was auto-queued.
///
/// Args retained for call-site stability. Effort is no longer rewritten under
/// economic mode, so the toast is always the plain auto-run copy.
pub fn auto_implement_toast_for(
    _raw_cmd: &str,
    _enqueued_cmd: &str,
    _economic_mode: bool,
) -> String {
    AUTO_IMPLEMENT_TOAST.to_string()
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

    /// Named contract: explicit `/implement --effort N` (or bare `effort N`)
    /// must be honored. Economic mode must not silently rewrite N down to 1.
    #[test]
    fn explicit_implement_effort_honored_when_economic_mode_on() {
        // Operator-reported path: effort 2 must stay 2 (not revised to 1).
        let effort2 = "/implement --effort 2 Linear freestanding contracts";
        assert_eq!(
            clamp_implement_effort_for_economic_mode(effort2, true),
            effort2,
            "economic mode must not downgrade explicit --effort 2 to 1"
        );
        assert_eq!(
            clamp_implement_effort_for_economic_mode(effort2, false),
            effort2
        );

        let high = "/implement --effort 5 residual work:\n1) wire Systems.Proc";
        let got = clamp_implement_effort_for_economic_mode(high, true);
        assert!(
            got.starts_with("/implement --effort 5 "),
            "explicit --effort 5 must stay 5 under economic mode: {got}"
        );
        assert!(got.contains("1) wire Systems.Proc"));
        assert_eq!(clamp_implement_effort_for_economic_mode(high, false), high);

        let low = "/implement --effort 1 fix tests";
        assert_eq!(clamp_implement_effort_for_economic_mode(low, true), low);

        let bare = "/implement effort 3 do the thing";
        assert_eq!(
            clamp_implement_effort_for_economic_mode(bare, true),
            bare,
            "bare effort form must also be honored: {}",
            clamp_implement_effort_for_economic_mode(bare, true)
        );

        let none = "/implement fix the gate";
        assert_eq!(clamp_implement_effort_for_economic_mode(none, true), none);
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
        // Explicit effort is not rewritten under economic mode, so the toast
        // stays the plain auto-run copy (no "economic mode: --effort capped").
        let raw = "/implement --effort 5 residual";
        let enqueued = clamp_implement_effort_for_economic_mode(raw, true);
        assert_eq!(enqueued, raw);
        assert_eq!(
            auto_implement_toast_for(raw, &enqueued, true),
            AUTO_IMPLEMENT_TOAST
        );
        assert_eq!(
            auto_implement_toast_for(raw, raw, true),
            AUTO_IMPLEMENT_TOAST
        );
    }
}
