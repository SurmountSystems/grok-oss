# Wave-1 health (pager-red / image-strip)

Scout: L3, read-only. Written 2026-08-15T16:21:40-06:00.
Workspace: `/home/hunter/Projects/surmount/grok-build`.
No product edits. No processes killed.

## Named reports

| Path | Present | Bytes | Lines | mtime |
|------|---------|-------|-------|-------|
| `.agents/reports/bug-pager-selection-render-red.md` | yes | 11471 | 134 | 2026-08-15 16:06:22.198292230 -0600 |
| `.agents/reports/bug-poisoned-image-session-recovery.md` | **no** | — | — | — |
| `.agents/reports/fork-docs-finish-write.md` | yes | 5130 | 62 | 2026-08-15 16:16:45.741957607 -0600 |
| `.agents/reports/wave1-pager-image-ready.md` | **no** | — | — | — |

No file matching `.agents/reports/*image*`, `.agents/reports/*poison*`, `.agents/joins/*image*`, or `.agents/joins/*poison*` exists.

Related, not in the named list: `.agents/reports/wave1-reports-ready.md` exists (1875 bytes, 2026-08-15 16:05:38). That poll (cap ~12 min, ended 16:05:23) recorded pager-red and image-recovery as **missing**. Pager-red appeared 59 seconds later. Image-recovery never appeared. `.agents/reports/bug-pager-selection-render-green.md` is also missing.

### `bug-pager-selection-render-red.md` first 20

```
# Red diagnosis: pager width / selection / render cluster

Diagnosis only. No product edit in this turn.

These four tests are one cluster. They are not four unrelated bugs. They share one product root with two branches.

## Commands and env

```
cd /home/hunter/Projects/surmount/grok-build
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-pager --lib -- \
  table_copy_uses_width_snapshot_when_anchor_block_scrolled_out \
  message_block_content_width_subtracts_timestamp_reservation \
  overlay_pretty_link_url_with_cjk_text \
  test_selection_model_top_clipped_markdown_entry
```

Result: **4 failed, 0 passed**, 8889 filtered out, exit 101.
```

### `bug-pager-selection-render-red.md` last 20

```
Keep these green while fixing: `append_bubble_copy_button_paints_when_first_line_fills_content_width`, `bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width`, `clicking_wide_human_bubble_copy_still_paints_and_copies`, `clicking_assistant_bubble_copy_copies_the_message`.

## Files and functions for the implementer

| Path | What to touch |
|------|----------------|
| `crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs` | `append_bubble_copy_button` |
| `crates/codegen/xai-grok-pager/src/scrollback/blocks/agent.rs` | `AgentMessageBlock::output` (call site) |
| `crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs` | `UserPromptBlock::output` (same helper; human full-width click tests) |
| `crates/codegen/xai-grok-pager/src/scrollback/wrappers/entry_renderer.rs` | Paint `⧉` in pad / timestamp gutter when content slack is gone |
| `crates/codegen/xai-grok-pager/src/scrollback/types.rs` | `BlockLine::copy_button_col`, `bubble_copy_button_rect` if the overflow column lives past content width |
| `crates/codegen/xai-grok-pager/src/scrollback/render.rs` | `map_hyperlinks_to_overlay` only if content width is still inflated |
| `crates/codegen/xai-grok-pager/src/app/agent_view/selection.rs` | `with_entry_output_text_source` only if detect still sees a hole |
| `crates/codegen/xai-grok-pager/src/scrollback/table_geometry.rs` | `TableGeometry::detect` only if a remaining chrome line sits inside the grid |

Timestamp reservation in `timestamp_reserved_for_block` / `EntryRenderer::timestamp_reserved` does not need a behavior change for this cluster.

## Stop

Red is observed. Product fix is not done in this turn.
```

The red report is a finished diagnosis. It does not claim a product fix.

### `fork-docs-finish-write.md` first 20

```
# FORK docs finish write

**Date:** 2026-08-15
**Role:** L3 docs finisher. Docs only. No product `*.rs`. No new cargo tests.

Map: `.agents/reports/fork-docs-finish-map.md`

User-guide paths exist exactly where the map named them under `crates/codegen/xai-grok-pager/docs/user-guide/`. No path surprise.

---

## Leftovers 1–8

| Item | Status | What changed |
|------|--------|--------------|
| 1. `03-keyboard-shortcuts.md` FORK pin | **Done** | Added plan keys `a` / `A` / `?` / `s` / `q` and "Empty Enter never approves a plan." Named the composer footer Enter cue as send / queue / interject. Relabeled `Ctrl+Enter` / Apple `Ctrl+O` / VS Code `Ctrl+L` as soft interject (inject, never cancel). Cancel is Esc / `[stop]` only. Removed the old "Send now (cancels the current turn)" wording for those keys. |
| 2. `16-subagents.md` soft interject | **Done** | One pin next to the worktree-default paragraph: mid-turn interject injects and never cancels. Cancel is Esc / `[stop]` only. Points at `03-keyboard-shortcuts.md`. |
| 3. `22-permissions-and-safety.md` plan Approve | **Done** | Next to the Always-approve definition: always-approve skips tool-permission prompts only. It does not click plan Approve. Links `19-plan-mode.md`. |
| 4. FORK class 5 `/limits` mix | **Done** | Moved `show_limits`, `format_supergrok_session`, `footer_names_live_principal`, `limits_json_lists_two_supergrok_principals_when_both_slots_exist`, `limits_json_honest_single_supergrok_session_cannot_see_team_plan` out of `# 5.` into the neighbor cargo block. Class 5 pager line is hop 5b compact-meter names only. |
| 5. FORK dead identifiers | **Done** | Same-batch Product bullet now names `same_batch_plan_write_before_exit_plan_mode_returns_new_body`. Soft-interject bullet dropped `enter_prompt_mode`; footer cue is shipped in code with no named footer `fn`; never-cancel stays `interject_contract_*`. Dogfood cargo deleted no-`fn` names (`enter_prompt_mode_matrix`, `ctrl_c_dismisses_rewind`, `split_tool_batch_before_exit_plan_mode`, `credentials_rejected`, and the other dead plan-panel identifiers). Kept live / prefix-safe wave filters. Snapshot stays dated and is not required land. |
```

### `fork-docs-finish-write.md` last 20

```
| rustc 1.97.1 / fenix match | File pin only. Still not cargo land. |
| Empty `models_cache.json` miss | Still a code miss. Catalog extra now says so. No dedicated `fn`. |
| Nucleo pool `Some(2)` | Constant only. Catalog extra now says so. Reuse-per-root remains the proven cargo. |
| User-guide `/limits` hit-count; last-session guide sentences; three-layer guide paragraph | Text exists. No dedicated cargo pin. |
| Stuck-retry **pager** chrome (`retry_chrome_*`, `clip_retry_reason_*`, `retrying_*`) | No matching `fn`. Neighbor comment still forbids adding those identifiers. |
| `shell_collision` / pager `SHELL_RESERVED` | `fn` gone. Not re-listed. |
| `default_title_items_include_agents`, `title_escape_never_empty_payload`, `title_updates_gated_only_by_title_enabled` | No matching `fn`. Neighbor comment still forbids them. |
| Lower-left throbber **color** | Absent. Not enrolled. |
| Token Economy / economic-mode / auto-run `/settings` GUI rows | Not re-proven. |
| Session recap / cancel-subagents Settings e2e; `[subagents] allow_worktree` actually changing spawn isolation | Copy `fn` only. Unchanged. |
| Host `~/.agents/skills` as a product land class | It is not. |
| Live TUI / dogfood of a rebuilt `grok-oss` | Operator-gated. Dogfood section still says so. |
| Composer footer Enter cue (`enter_prompt_mode` / `enter_prompt_mode_matrix`) | UNPROVEN as a named test. FORK now says shipped in code, no named footer `fn`. Soft interject never-cancel is proven by `interject_contract_*` only. |
| Dead dogfood identifiers from leftover 5 | Removed from the dated snapshot. Not put in Required land. |

---

## Path surprise

None. The three user-guide pages live at the paths the map named.
```

The docs-finish report is a finished write-up (leftovers marked Done; ends at path surprise).

## Cargo / rustc / nextest

Target dir exists: `/home/hunter/.cache/grok-build-target` (dir mtime 2026-08-15 13:44:28).

Three snapshots:

1. **16:19:21** — no `cargo`, `rustc`, or `nextest` process. No readable `/proc/*/environ` with `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`. No cmdline matching `xai-grok-pager` or `xai-grok-shell` except this scout's own shell text.

2. **16:19:52 through 16:20:35** — one compile/test job appeared, then advanced:

   | When | PID | PPID | Elapsed | Process |
   |------|-----|------|---------|---------|
   | 16:19:52 | 2738011 | 2737990 | 00:11 | `cargo` |
   | 16:19:52 | 2738741 | 2738011 | 00:02 | `rustc --crate-name xai_grok_shell` |
   | 16:20:15 | 2738011 | 2737990 | 00:34 | same `cargo` |
   | 16:20:15 | 2738741 | 2738011 | 00:25 | `rustc --crate-name xai_grok_shell` (98.6% CPU) |
   | 16:20:35 | 2738011 | 2737990 | 00:55 | same `cargo` |
   | 16:20:35 | 2739679 | 2738011 | 00:19 | `rustc --crate-name xai_grok_pager` |

   Full cargo command (no secrets):

   ```
   /home/hunter/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin/cargo test -p xai-grok-pager --lib -- table_copy_uses_width_snapshot_when_anchor_block_scrolled_out message_block_content_width_subtracts_timestamp_reservation overlay_pretty_link_url_with_cjk_text test_selection_model_top_clipped_markdown_entry
   ```

   Parent bash PID 2737990 (`GROK_AGENT=1`) invoked:

   ```
   CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target TMPDIR=/home/hunter/.cache/grok-oss-tmp cargo test -p xai-grok-pager --lib -- …
   ```

   cwd: `/home/hunter/Projects/surmount/grok-build`.
   env: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`, `CARGO_HOME=/home/hunter/.cargo`.
   Grandparent: `grok-oss` PID 2093392, elapsed 17:36:35 at 16:20:15.
   This is the same four-test filter as the already-written red report. No `nextest`. No second cargo.

3. **16:20:55 and 16:21:40** — `ps -C cargo,rustc,rustdoc,clippy-driver,nextest` empty. Bash 2737990 gone. No live compile.

## `.cargo-lock`

`/home/hunter/.cache/grok-build-target/.cargo-lock` was **absent** on every check (16:19:21, 16:20:15, 16:20:35, 16:20:55, 16:21:40), including while cargo 2738011 was compiling.

`fuser` each time: `Specified filename /home/hunter/.cache/grok-build-target/.cargo-lock does not exist.`
Not held. Nothing to attach a PID to.

## Newest files in `.agents/reports/` (`ls -lt | head -20` at 16:21:40)

```
total 3292
-rw-r--r-- 1 hunter hunter  5130 2026-08-15 16:16:45  fork-docs-finish-write.md
-rw-r--r-- 1 hunter hunter 11471 2026-08-15 16:06:22  bug-pager-selection-render-red.md
-rw-r--r-- 1 hunter hunter  1875 2026-08-15 16:05:38  wave1-reports-ready.md
-rw-r--r-- 1 hunter hunter  3727 2026-08-15 16:01:31  bug-workspace-daemon-takeover-flaky.md
-rw-r--r-- 1 hunter hunter 14699 2026-08-15 16:00:24  fork-docs-finish-map.md
-rw-r--r-- 1 hunter hunter  6349 2026-08-15 15:46:31  fork-docs-defend-upstream.md
-rw-r--r-- 1 hunter hunter  5223 2026-08-15 15:45:27  fork-docs-mop.md
-rw-r--r-- 1 hunter hunter  2692 2026-08-15 15:42:15  fork-docs-fix.md
-rw-r--r-- 1 hunter hunter 13284 2026-08-15 15:37:27  fork-docs-review.md
-rw-r--r-- 1 hunter hunter  6142 2026-08-15 15:28:21  fork-docs-process-write.md
-rw-r--r-- 1 hunter hunter  6533 2026-08-15 15:23:16  fork-docs-fork-write.md
-rw-r--r-- 1 hunter hunter 42464 2026-08-15 15:10:50  fork-docs-seams.md
-rw-r--r-- 1 hunter hunter 19572 2026-08-15 15:07:06  fork-docs-tools-map.md
-rw-r--r-- 1 hunter hunter 14302 2026-08-15 15:06:22  fork-docs-process-gaps.md
-rw-r--r-- 1 hunter hunter 19080 2026-08-15 15:03:55  fork-docs-map.md
-rw-r--r-- 1 hunter hunter  6939 2026-08-15 14:49:37  bug-bubble-copy-leftovers.md
-rw-r--r-- 1 hunter hunter  1989 2026-08-15 14:48:51  bug-bubble-copy-leftovers-mop.md
-rw-r--r-- 1 hunter hunter  7399 2026-08-15 14:46:51  bug-bubble-copy-leftovers-impl-ready.md
-rw-r--r-- 1 hunter hunter  7399 2026-08-15 14:46:04  bug-bubble-copy-leftovers-impl.md
-rw-r--r-- 1 hunter hunter 42434 2026-08-15 14:38:47  bug-bubble-copy-leftovers-snapshot.md
```

Newest wave-1 artifact after the red report is docs-finish (16:16:45). Nothing newer from pager-red or image-strip.

## Other live waiters under `grok-oss` 2093392 (not cargo)

Observed 16:20:55; rechecked 16:21:40.

| PID | Elapsed at 16:21:40 | What it is doing |
|-----|---------------------|------------------|
| 2728092 | 13:59 | Loop: sleep 20 until **both** pager-red and image-recovery reports exist and are `>800` bytes. Cap `SECONDS+1080` (~18 min). Child: `sleep 20`. Image file still missing, so this loop is still waiting. |
| 2737854 | 02:15 | Loop: sleep 25 until `.agents/reports/bug-pager-selection-render-green.md` exists and is `>800` bytes. Cap 1500 s. Child: `sleep 25`. Green file still missing. |
| 2739449 | gone by 16:21:40 | Was `date; sleep 45; ls -la …/bug-poisoned-image-session-recovery.md`. |

These are pollers. They are not compiling.

## Health

**Pager-red L3:** the named red report is on disk, 11471 bytes, last lines say red was observed and the product fix was not done in that turn. mtime 16:06:22. No later rewrite. At scout time there is **no** live `cargo`/`rustc` for that crate. A later job (cargo 2738011, ~16:19:41–16:20:55) re-ran the **same four-test filter** and compiled `xai_grok_shell` then `xai_grok_pager`, then exited. That job is gone. The red-diagnosis report already existed before that compile. From reports plus process table: the red diagnosis write is done; compile work seen after that was a short re-run of the same red tests, now idle.

**Image-strip L3:** the named recovery report is **missing**. `wave1-pager-image-ready.md` is **missing**. No `*image*` / `*poison*` report under `.agents/reports` or `.agents/joins`. No `cargo`/`rustc`/`nextest` compiling image-session work. Only a waiter (2728092) still polling for that file. This scout cannot see a live image-strip compiler. Whether the L3 is still thinking inside `grok-oss` 2093392 is not visible from process or report files. Disk and cargo: **no output, not compiling**.

**Lock:** not present, not held.

**`wave1-pager-image-ready.md`:** still missing. Wave-1 image half has no ready marker.
