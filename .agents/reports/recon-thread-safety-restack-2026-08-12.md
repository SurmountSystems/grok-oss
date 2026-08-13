# Thread-safety restack preference (join then 1.0.3)

Read-only inventory for: (1) `merge -s ours` of Surmount `main` into `onto-xai/b13fa526f511` (tree stays onto tip; no file choice), then (2) put-history restack onto public Grok Build 1.0.3 (`xai-org/main` = `e5fd4816d432`, already fetched). Compared this working tree to that tip via GitHub raw files. Changelog: [Grok Build Changelog](https://x.ai/build/changelog) (accessed: 2026-08-12).

Do **not** start join or put-history from this note.

## What must survive restack (Surmount nucleo contract)

1. **Reuse-per-root.** `FuzzySearchManager::open` must return the existing search id for the same `root` path. It must not call `FuzzySearchContext::new` (and therefore must not call `Nucleo::new(..., Some(2), 1)`) again for that root.
2. **`Some(2)` never `None`.** `Nucleo::new` must pass `Some(NUM_NUCLEO_THREADS)` with `NUM_NUCLEO_THREADS = 2`. `None` builds one rayon pool sized to `available_parallelism()` per matcher.
3. **Poll does not refresh `last_activity`.** `get_results` and `get_results_filtered` are `&self` reads. Query `change` and a reused `open` still bump the timer. Tests: `repeated_open_without_close_keeps_one_search_per_root`, `distinct_roots_each_keep_one_search`, `get_results_does_not_keep_a_stale_search_alive`.

## Inventory in this tree

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-workspace/src/file_system/fuzzy.rs` | Only `Nucleo::new` site. `Some(2)` workers, degrade-if-no-threads, daemon **joins** on Drop. |
| `crates/codegen/xai-grok-workspace/src/file_system/mod.rs` | `FuzzySearchManager` reuse-per-root + poll-does-not-keep-alive (shipped 2026-08-12). |
| `crates/codegen/xai-grok-workspace/src/handle.rs` | Manager timeout **300s**; `fuzzy_poll` / `fuzzy_get_results` use `&self` getters. |
| `crates/codegen/xai-grok-workspace/src/session/mod.rs` | Remote/ACP manager holder. |
| `crates/codegen/xai-grok-pager/src/views/file_search/state.rs` | TUI `@`. Lazy daemon on first `@`. One Nucleo per `FileSearchState`. |
| `crates/codegen/xai-grok-pager/src/views/history_search.rs` | Prompt history. **Eager** `Daemon::new()` in `HistorySearchState::new()`. Drop sends `Stop` and **does not join**. Not a nucleo pool (`Matcher` on one `history-search` thread). |
| `crates/codegen/xai-grok-pager/src/slash/matcher.rs` | Slash palette. In-process `Matcher`. No pool. |
| `crates/codegen/xai-grok-shell/src/extensions/suggest/file_provider.rs` | Sync ranker. No pool. |
| `crates/codegen/xai-tty-utils/src/runtime.rs` | Workspace/TUI tokio cap: max **8** workers, **16** blocking. |
| `crates/codegen/xai-grok-workspace/src/bin/workspace_server.rs` | Uses `capped_worker_threads()`. |
| `crates/codegen/xai-gix-status/src/lib.rs` | gix status produce-worker cap (never `Some(0)`). Already in this tree. |
| `crates/codegen/xai-grok-workspace/src/file_system/git_status.rs` | Compact git status via CLI + ODB permit. |
| `crates/codegen/xai-grok-tools/src/implementations/grok_build/task/admission.rs` | Bounded subagent spawn (default 32, queue). Already matches 1.0.3 text. |

Not nucleo: session picker, project picker, ripgrep content search, fsnotify.

## 1.0.3 vs this tree (tip `e5fd4816`)

**Fuzzy matcher crate split.** 1.0.3 moved `FuzzyFileMatcher` to new crate `crates/codegen/xai-fuzzy-file-search/src/lib.rs`. The matcher body is the same as our `fuzzy.rs`: `Some(2)`, probes, daemon join-on-drop. Workspace `file_system/mod.rs` re-exports `xai_fuzzy_file_search::*`. This tree does **not** have that crate yet.

**`FuzzySearchManager` on 1.0.3 still leaks nucleo.** Their `open` always inserts a new `FuzzySearchContext`. Their `get_results` / `get_results_filtered` take `&mut self` and **write `last_activity`**. No reuse-per-root tests. **Do not take their manager body.** After accepting the crate split, re-apply our `open` / `&self` getters / three tests.

**History search (changelog 1.0.1).** They did **not** add join-on-drop. `Daemon::drop` still only `send(Stop)`. Their leak fix is **lazy spawn**: `daemon: Option<Daemon>`, `ensure_daemon()` on first `activate`, `construction_does_not_spawn_the_daemon` test. Eager `new()` on every `PromptWidget` (including never-removed subagent child views) was the leak. **Take their lazy-spawn design.** Join-on-drop is optional extra (mirror `FuzzyFileMatcherDaemon`); it is not what 1.0.3 shipped.

Their `activate` / `activate_browse` return `bool`. Call sites in `app_view.rs` and `agent_view/input.rs` must be updated if we take that signature. They also `SetItems(Vec::new())` on deactivate (history released while overlay closed). We should keep that when taking their file.

**FileSearchState.** Both trees already lazy-build `@` daemons. Take 1.0.3 if the pick moves the type; keep one daemon per state, not per keystroke.

**Bounded subagent spawn (1.0.1).** Already here. Take 1.0.3 `admission.rs` if it drifted; do not drop Queue default.

**Faster spawn with many `~/.grok` sessions (1.0.3).** New crate `xai-grok-active-sessions` (and `xai-grok-session-search`). Not in this tree. Take those crates.

**Workspace daemon workers.** Policy already here (`capped_worker_threads`, max 8). 1.0.3 also extracts `xai-grok-workspace-daemon`. Take the crate layout; keep the 8/16 caps. Never let a pick restore unbounded tokio workers.

**Git status CPU (1.0.1).** `xai-gix-status` already here. Take 1.0.3 if the pick is newer. Keep the hard cap of 8 and never pass `Some(0)` to gix.

## Conflict-preference table (restack implementer)

| File / crate | Preference | Why |
|--------------|------------|-----|
| `file_system/mod.rs` `FuzzySearchManager` | **Keep Surmount** (re-apply after crate split) | 1.0.3 still one Nucleo per `open` and poll keeps searches alive. |
| `file_system/fuzzy.rs` vs new `xai-fuzzy-file-search` | **Take 1.0.3 crate**, keep `Some(2)` | Same matcher; new home. Verify `Some(NUM_NUCLEO_THREADS)` after pick. |
| `handle.rs` fuzzy getters | **Keep Surmount `&self` poll** | Their getters are `&mut` and refresh `last_activity`. Keep 300s timeout unless they add a timer sweep we want. |
| `views/history_search.rs` | **Take 1.0.3 lazy spawn**, keep browse/search UX | Their fix is lazy first-activate, not join. Merge `activate` bool + `SetItems` on deactivate. |
| `views/file_search/state.rs` | **Take 1.0.3 if moved**, keep lazy first-`@` | Not the nucleo storm. |
| `task/admission.rs` | **Take 1.0.3** (already same) | Bounded fan-out already shipped. |
| `xai-grok-active-sessions`, `xai-grok-session-search` | **Take 1.0.3** | New; faster spawn with many sessions. |
| `xai-grok-workspace-daemon`, `workspace_server.rs`, `xai-tty-utils/src/runtime.rs` | **Take 1.0.3 layout**, keep 8/16 caps | Worker-limit product. |
| `xai-gix-status`, `file_system/git_status.rs` | **Take 1.0.3** | Git status/diff CPU. Keep nproc cap. |
| Slash / shell `Matcher` rankers | **Take 1.0.3** | No nucleo pool. |

## Join vs restack

Join (`merge -s ours`) does not rewrite these files. All preference rows apply to **put-history cherry-picks** onto `e5fd4816`. After restack, re-run:

```
cargo test -p xai-grok-workspace --lib file_system::tests -- --nocapture --test-threads=1
```

(and the history-search construction / reuse tests once they live on 1.0.3's lazy API). If the matcher moved crates, point the filter at `xai-fuzzy-file-search` plus workspace manager tests.
