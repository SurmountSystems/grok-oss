# Implement: killall cancel-resume forensic fix (2026-08-08)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Binary installed:** `~/.cargo/bin/grok-oss` via `just install` (0.2.111)

## Phase 1: Live forensic (this machine)

| Check | Result |
|-------|--------|
| Installed binary (pre-this-fix) | `grok-oss 0.2.111 (c87f66a61d94) [stable]`, mtime **2026-08-08 02:23** local |
| All `canceled_turn_resume.json` under `~/.grok/sessions` | 5 files: 1 live grok-build parent, 4 `/tmp` test leftovers |
| **Iso project tree** `~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fai%2Fiso/**` | **Zero markers** (no `canceled_turn_resume*` anywhere) |
| Project-local session home under `~/Projects/ai/iso` | None (only `.agents/`) |
| Hot iso session | `019f85f6-3971-7363-a8b6-833ed66829c0` (fork, last activity ~04:39–04:55 UTC implement wave) |
| Unfinished child under that parent | `subagents/019fdfaa-e254-7250-8414-03d207730650` has **meta.json only** (no `output.json`); standalone child session also **no marker** |
| Unified log for iso sid | Only `session.load.start/done` at **04:56** and **07:11** UTC (reopen idle). No cancel-resume apply lines. |
| Live proof eager write works on freeform parent | Parent `019faf9d-…` (grok-build) had mid-turn marker `prompt_text="Didn't work. [Image #1]"` written **08:28:07Z** while turn + child implementer ran |

### Critical question: does the iso session they reopen have a marker **now**?

**No.** Every iso session dir (including the hot one) is **MARKER NO**.

| Outcome | Means for iso dogfood |
|---------|------------------------|
| No marker | Resume path never had a one-shot file to load. Reopen = cold history, empty composer, no toast. |

### Timing (why prior “eager write” install did not save that iso wave)

| Event (UTC) | What |
|-------------|------|
| ~04:39 | Iso `/implement` spawned unfinished child `019fdfaa-…` |
| 04:56, 07:11 | Operator reopened iso: `session.load.*` only, idle |
| **08:23** (02:23 local) | Binary with prior eager write + force-drain install landed |

Both dogfood reopens of iso **predate** that install. Reopening an old session that never got a marker cannot invent resume. Operator still needs a **new mid-turn** on the fixed binary, then killall, then reopen.

## Phase 2: Code path map

| Path | Arms cancel-resume? | Notes |
|------|---------------------|--------|
| Local `maybe_drain_queue` `QueueEntryKind::Prompt` (chat + **InjectSkill `/implement`**) | **Yes** | `note_cancel_resume_prompt_text(&queued.text)` after `start_turn_boundary`; display text, not raw skill XML |
| Turn-start shim (server-owned queue adoption) | **Yes** when text present | Same note on restore_text |
| Bash / compact command drain | No | Not implement dogfood |
| Session open → `handle_session_loaded` | Applies marker | Cold-load zombie finalize + **force drain** + re-warm if Send fails (prior fix kept) |
| Successful PromptResponse / reconcile | **Was always clear** | Hole when parent finished while **live background subagents** still ran |

Skill/slash `/implement` does **not** skip arming on drain. The dogfood hole after eager write was:

1. **No marker on disk** for the iso wave (binary/install timing + killall before durable write era), and
2. **Clear-on-success while children live**: parent implement PromptResponse can land with unfinished background implementers → marker deleted → killall mid-child → reopen idle.

## Phase 3: Fix (this turn)

1. **`finalize_cancel_resume_after_successful_turn`** (`dispatch/turn.rs`): on clean success, **keep/re-write** the parent prompt marker when `holds_queue_for_background()`; otherwise clear. Captures whole-turn text **before** `finish_turn`. Wired from PromptResponse (`prompt.rs`) and end-reconcile (`turn.rs`).
2. **Info logs**: marker written (eager / signal arm / cancel-quit), kept after success+children, cleared after clean success, applied on session load, drain started vs blocked.
3. Prior contracts kept: eager note at turn start, force SendPrompt on load with zombies, re-warm if drain fails.

## Phase 4: TDD

| Test | Contract |
|------|----------|
| `skill_inject_drain_eagerly_writes_cancel_resume_marker` | **New:** InjectSkill-shaped queue row (`wire_blocks` + `display_as_skill`) drains → marker with **display** `/implement …` text |
| `successful_turn_with_live_subagents_keeps_cancel_resume_marker` | **New:** live background child → finalize keeps marker |
| `successful_turn_without_live_subagents_clears_cancel_resume_marker` | **New:** clean success clears |
| `note_cancel_resume_eagerly_writes_durable_marker_without_quit` | Regression |
| `session_loaded_applies_cancel_resume_marker_and_toasts` | SendPrompt + toast |
| `session_loaded_cancel_resume_starts_turn_despite_zombie_subagents` | Force drain past zombies |
| `quit_mid_turn_*` / shell `canceled_turn_resume` | Green |

```bash
cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo test -p xai-grok-pager --lib -- \
  skill_inject_drain successful_turn_with_live successful_turn_without_live \
  note_cancel_resume session_loaded_applies_cancel session_loaded_cancel_resume \
  quit_mid_turn quit_idle_does_not_write
cargo test -p xai-grok-shell --lib -- canceled_turn_resume
cargo clippy -p xai-grok-pager -p xai-grok-shell --lib -- -D warnings
just install
```

All listed filters green; clippy `--lib` clean; install verified.

## Phase 5: Operator retest recipe

**Important:** Reopening the **old** iso session without a marker will still look idle. Prove the write path on a **live** turn with the new binary:

```bash
# 1. Install is already done:
~/.cargo/bin/grok-oss --version

# 2. In the target project (e.g. ~/Projects/ai/iso), start a real turn:
#    /implement something long enough to stay mid-tool / mid-subagent

# 3. WHILE THE TURN IS RUNNING (before killall), confirm the marker:
find ~/.grok/sessions -name canceled_turn_resume.json
# Must include a path under the session you are in, e.g.:
# ~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fai%2Fiso/<session-id>/canceled_turn_resume.json
#
# Keys only (no secrets dump):
python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print({k:(len(v) if k=='prompt_text' else v) for k,v in d.items()})" \
  ~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fai%2Fiso/<session-id>/canceled_turn_resume.json

# 4. Kill hard:
sudo killall grok-oss

# 5. Reopen the SAME session (cwd last-session or --resume <session-id>)

# 6. Expect:
#    - toast: Resuming canceled turn...
#    - interrupted prompt auto-starts (spinner / tools), not empty idle composer
```

If step 3 shows **no** marker while a parent turn is running on this binary, the write path is still broken; collect unified log lines containing `canceled_turn_resume`.

### Key paths

| Concern | Path |
|---------|------|
| Eager arm + info log | `xai-grok-shell/.../canceled_turn_resume.rs` |
| Skill/chat drain note | `xai-grok-pager/.../dispatch/queue.rs` |
| Keep marker when children live | `xai-grok-pager/.../dispatch/turn.rs` (`finalize_cancel_resume_after_successful_turn`) |
| PromptResponse wire-up | `dispatch/prompt.rs` |
| Load apply + force drain + zombies | `dispatch/session/load.rs` |
