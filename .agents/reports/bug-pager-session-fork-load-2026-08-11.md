# Session fork + load residual

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Prior:** `.agents/reports/bug-pager-session-lifecycle-2026-08-11.md` § Not in this slice
**Agent:** L2 implementer

---

## Status

| Item | Value |
|------|--------|
| **Sample baseline (session module)** | **263 pass / 14 fail** (lib compiles) |
| **Fork reds** | **3** (`startup_fork_parent_is_worktree…`, `worktree_forked_clears_sticky_branch…`, `fork_session_ready_sticky_chat…`) |
| **Load reds** | **9** (title hydrate cold-cache/sanitize, last-turn cold-cache/rewind, standalone worktree mark, sticky chat restore) |
| **Out of scope in sample** | 2 lifecycle dashboard-stop (other agents; full session re-run later **0** fail) |
| **Product fix** | Monorepo tip contracts restored (`b13fa526` paths) |
| **Verify** | fork **59/59**, load **81/81**, full `app::dispatch::tests::session::` **277 pass / 0 fail** |
| **fmt** | `cargo fmt -p xai-grok-pager` |

---

## Sample red (this slice)

### Fork (3)

| Test | Assert |
|------|--------|
| `startup_fork_parent_is_worktree_for_standalone_clone` | `parent_is_worktree: true` for CoW clone with `.git/grok-worktree-source` |
| `worktree_forked_clears_sticky_branch_from_main_repo` | clear `current_branch` / `main_repo`, set `agent.is_worktree` |
| `fork_session_ready_sticky_chat_sets_rename_kind_chat` | sticky `--chat` stamps `conversation_entry` (rename kind Chat) |

### Load (9)

| Test | Assert |
|------|--------|
| `session_title_hydration_manual_restores_display_name_cold_cache_only` | title fields cold-cache only |
| `session_title_hydration_does_not_clobber_live_generated_title` | live auto title wins over late disk |
| `session_title_hydration_skips_control_only_title` | control-only → skip |
| `session_title_hydration_sanitizes_and_caps_dirty_title` | strip controls + scalar cap |
| `last_turn_summary_hydration_is_cold_cache_only` | live delivery not overwritten |
| `last_turn_summary_hydration_does_not_restore_after_rewind_clear` | gen bump blocks pre-rewind disk |
| `load_session_marks_standalone_worktree_cwd` | resume cwd with source marker → `session.is_worktree` |
| `remote_restore_marks_standalone_worktree_cwd` | same for restore path |
| `session_restored_sticky_chat_sets_conversation_entry` | sticky `--chat` restore stamps rename kind |

---

## Roots fixed (product)

Tests encode monorepo contracts; product restored from tip `b13fa526` (no expect rewrites).

### 1. `parent_session_is_worktree` — standalone CoW clones

**Root:** When `.git` was a directory the helper returned `false`, so standalone grok worktrees (directory `.git` + `grok-worktree-source`) were not worktree-backed for startup fork or load.

**Fix (`session_startup.rs`):**

- Honor `worktree_label` in `summary.json`.
- Prefer `git_info::compute_cwd_git_info(cwd).is_worktree`.
- Fallback walk: `.git` file → true; `.git` dir → true only when `grok-worktree-source` is non-empty.

### 2. Load / restore mark `session.is_worktree`

**Root:** `dispatch_load_session_ungated` and `dispatch_load_session_with_restore` hard-coded `is_worktree: false`.

**Fix (`load.rs`):** set via `parent_session_is_worktree(&session_id, &cwd)`.

### 3. Sticky `--chat` → `conversation_entry` / rename kind

**Root:** Fork ready / worktree forked / session restored / load only set UI `chat_kind`, not rename-kind `conversation_entry`.

**Fix:**

- Added `session_opens_as_chat(app, chat_kind)` in `load.rs` (picker chat row, else sticky `--chat`; `local-workspace` history-as-build forces false).
- Stamp `agent.conversation_entry` on load, restore-with-load, `SessionRestored`, `ForkSessionReady`, `WorktreeForked`.
- Effect `LoadSession.chat_kind` stays the raw conversation-entry bit (not OR sticky).

### 4. Worktree forked clears sticky main-repo git chrome

**Root:** `handle_worktree_forked` set `session.is_worktree` only.

**Fix (`fork.rs`):** same as worktree session create — clear `current_branch` / `main_repo`, set `agent.is_worktree = true`.

### 5. `SessionMetaFromDisk` hydrate contracts

**Root:** Always overwrote title + last-turn summary; no sanitize/cap.

**Fix (`task_result.rs`):**

- `sanitize_and_cap_title` (control-only → skip; dirty → strip + cap).
- `display_name` only when manual and cold (`display_name.is_none()`).
- `generated_session_title` only when cold (`is_none()`).
- Last-turn summary only when `last_turn_summary_gen` still matches enqueue gen **and** field is `None` (live set / rewind bump win).

---

## Files changed

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-pager/src/app/session_startup.rs` | standalone worktree detect |
| `crates/codegen/xai-grok-pager/src/app/dispatch/task_result.rs` | SessionMetaFromDisk cold-cache + sanitize |
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` | `session_opens_as_chat`, worktree mark, conversation_entry stamps |
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/fork.rs` | sticky branch clear, conversation_entry, import `session_opens_as_chat` |

No FORK dual-pin (intent matches monorepo tests; no fork-doc lie).

---

## Verify

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'app::dispatch::tests::session::fork::' -- --test-threads=8
# → 59 passed; 0 failed

nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'app::dispatch::tests::session::load::' -- --test-threads=8
# → 81 passed; 0 failed

nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'app::dispatch::tests::session::' -- --test-threads=8
# → 277 passed; 0 failed

nice -n 19 ionice -c3 cargo fmt -p xai-grok-pager
```

**Baseline → after:** session module **263/14** → **277/0** (fork 3 + load 9 fixed; prior lifecycle residual and sample dashboard-stop also green on re-run).

---

## Out of scope (left alone)

- Dashboard stop double-press / peek selection (lifecycle; other agents).
- Status / turn large rewrites.
- Concurrent half-merge compile noise in other crates during mid-turn (cleared by re-run).
