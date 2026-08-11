# Diagnosis: `/rebuild` TUI glitch (`bug:rebuild-tui-glitch`)

**Date:** 2026-08-07
**Mode:** light code explore after Wave A–C green. **No product fix** this run (no TTY dogfood of live multi-TUI `/rebuild`, no observed red).
**Board:** leave **open**.

---

## What shipped and is unit-green

| Seam | Path | Unit proof |
|------|------|------------|
| SHA-aware relaunch identity | `xai-grok-shell` leader | `leader_is_older_than`, `parse_binary`, `decide_relaunch` |
| Shared rebuild core | `xai-grok-update::rebuild` | `rebuild::` (4) |
| Slash + Action | `xai-grok-pager` slash/dispatch | `slash::commands::rebuild`, `dispatch::rebuild` |
| Self re-exec after quit | `dispatch/rebuild.rs` → `app/mod.rs` | struct holds paths; order: restore then exec |

Re-verified green in verify run 2026-08-07.

---

## Exit / paint order (code)

Successful `/rebuild` with an active session id:

1. Toast + scrollback summary of install.
2. If turn running: cancel with `canceled_turn_resume` marker.
3. Arm `app.rebuild_relaunch` (`session_id`, `installed_exe`, `minimal`).
4. `QuitConfirmed` so `active_sessions` unregister runs.
5. Event loop returns `RunResult { rebuild_relaunch: Some(...) }`.
6. **`restore_terminal(...)` runs first** (leave alt screen, drain writer, raw mode off).
7. Then `exec_rebuild_relaunch`: build `--resume` args (same pattern as screen-mode relaunch), **eprintln** "Relaunching on …", flush, **`Command::exec`** onto installed binary.

So the intended contract is: **terminal fully restored before re-exec**, same family as `/minimal` ↔ `/fullscreen` relaunch.

---

## Likely glitch classes (hypothesis only)

Without a captured PTY transcript, these are ranked suspects, not root cause:

1. **Double TUI lifecycle race**
   Invoking process self-re-execs **and** leaders may get `RelaunchForUpdate`. Sibling TUIs / leader drain can leave a pane half-restored or briefly double-init alt screen. Unit tests do not cover multi-process paint.

2. **stderr chatter after restore**
   `exec_rebuild_relaunch` always `eprintln!`s before `exec`. On some terminals that can flash a line between leave-alt-screen and new process init. Screen-mode relaunch may be quieter.

3. **Resume cold paint**
   New process `--resume` + screen mode env re-enters terminal. First frame after resume can look wrong (theme, size, incomplete clear) if writer thread / mouse / bracketed paste re-engage order differs from cold start. Existing PTY e2e covers SIGTERM restore and some resume, not specifically rebuild re-exec.

4. **Failed restore still re-execs**
   Code logs restore failure but still proceeds to rebuild relaunch on `Ok(run_result)`. A partial restore + exec could leave modes latched.

5. **No session id path**
   Install succeeds but no re-exec (message only). Different UX; not the paint glitch class.

---

## What would prove a fix (TDD)

| Step | Requirement |
|------|-------------|
| Red | In-tree test or PTY e2e that fails on the named paint contract (e.g. after rebuild relaunch path, raw mode off + leave alt screen ordered; or resume first frame has no double-enter without leave). Prefer existing `pty_e2e` / wrap_restore patterns. |
| Green | Minimal product change on restore or relaunch path. |
| Dogfood | Operator two-pane `/rebuild` once; confirm glitch gone. |

**Do not:** invent a clear-screen or sleep "fix" without a failing contract.

---

## Recommendation

1. Keep board item open.
2. Next implement slice: operator reproduces with notes (alt screen? minimal? multi-TUI?) → implementer encodes red PTY/unit → fix.
3. Soft compare path: `screen_mode_relaunch::exec_screen_mode_relaunch` vs `exec_rebuild_relaunch` (args, env, stderr noise) for intentional parity.

**Not done this run:** product edit or fake green claim that the glitch is closed.
