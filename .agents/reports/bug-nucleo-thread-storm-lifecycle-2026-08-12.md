# Nucleo thread lifecycle (accumulation)

Read-only pass. Spawn-site inventory is owned by another agent. This note is about when engines live, how their workers start, and what can keep them.

## What a `nucleo` thread is

The only `Nucleo::new` in this tree is `FuzzyFileMatcher::new` in `crates/codegen/xai-grok-workspace/src/file_system/fuzzy.rs`. It asks for a dedicated rayon pool of `NUM_NUCLEO_THREADS` (2). Nucleo names those OS threads `nucleo worker {i}` (`~/.cargo/git/checkouts/nucleo-425d994cd74b3654/5b74652/src/worker.rs`). They are not tokio tasks.

`Nucleo::drop` sets cancel, waits up to one second for the worker lock, then drops the rayon `ThreadPool` (which should join those two workers). Product `Drop` for `FuzzyFileMatcher` joins the ignore walk first. `FuzzyFileMatcherDaemon::drop` sends `Stop` and joins the `fuzzy-daemon` thread. If a `Nucleo` is still reachable, its two workers stay parked forever.

## Create vs drop (TUI / session / ACP)

**In-process `@` picker (`FileSearchState`).** Each `PromptWidget` owns one `FileSearchState`. Construction is lazy: the first `@` token calls `ensure_daemon`, which builds one `FuzzyFileMatcher` plus one `fuzzy-daemon` thread. Later keystrokes call `set_query` on that same daemon. Leaving `@` clears results and does **not** drop the pool. `retarget` (worktree attach, dashboard cwd change, peek-reply cwd change) replaces the state and drops the old daemon, then rebuilds only on the next `@`. Tests assert reuse, not drop-and-rebuild, while staying in `@`.

`PromptWidget` owners: welcome prompt, each top-level `AgentView`, each dashboard dispatch box, dashboard peek reply, and **every subagent child view**. Child views are inserted with `insert_subagent_view` and **never removed** (`subagent_views.remove` has no callers). Child `AgentView::new` does not start nucleo until that child composer uses `@`.

**Workspace / ACP manager (`FuzzySearchManager`).** `WorkspaceHandle` holds one manager with a **300 second** stale timeout (`handle.rs`; the type default is 30 seconds and is unused there). `FuzzySearchContext::new` always builds a full matcher and daemon and starts a walk. `open` inserts a new context (new UUID unless the client repeats `request_id`). `close` removes one id. `cleanup_stale` runs **only inside `open`**, not on a timer. `change` / `get_results` refresh `last_activity`, so a search that is still polled never expires. Empty-query `change` also `restart_walk` (reuse the same nucleo, new ignore walk).

The pager TUI does **not** send `x.ai/search/fuzzy/open`. Local `@` is in-process. The manager is for ACP / hub (`x.ai/search/fuzzy/open` in `xai-grok-shell/src/extensions/search.rs`). Comment in `workspace/src/session/mod.rs` still says the shell has its own manager. Grep finds only this one.

**Not nucleo pools.** Project picker is a static question list. Session list is `x.ai/session/list` plus local `fuzzy_matches_session`. Slash / command palette uses `Matcher` + `MultiPattern` on the UI thread (`slash/matcher.rs`). Shell file suggestions use `Matcher::new` per rank (no pool). History search uses one `history-search` thread and `Matcher`, not `Nucleo`. Scrollback search is regex/substring. File watchers (`fs_notify.rs`) update hunk tracker and codebase graph only. They do not rebuild a matcher.

**History daemon leak class (wrong name, same session-age shape).** `HistorySearchState::new` always `std::thread::spawn`s `history-search`. `Drop` sends `Stop` and does **not** join. If the 256-deep channel is full, `Stop` is dropped and the thread can outlive the widget. Every subagent view pays this thread for the life of the parent tab.

## Keystroke / render / watch

- Every prompt key or cursor move can call `update_file_search_context`. That does **not** build a second nucleo after the first `@`.
- Entering `@` always `restart_walk` (join old ignore walk, `nucleo.restart`, spawn `fuzzy-walk` plus up to 8 ignore workers). Leaving `@` does not tear down nucleo.
- `poll` / dashboard ticks (~4ms) only read the daemon snapshot. They do not construct.
- File-watch callbacks do not call `Nucleo::new`.

## Healthy (~48 to 82) vs fat (~6k to 7k)

A quiet session with a few tabs and no `@` should have zero nucleo workers. After one `@`, expect two `nucleo worker` threads plus `fuzzy-daemon` and a walk that should end. Seven processes multiply that, they do not share one pool.

~7000 threads named nucleo means on the order of **3500 live `Nucleo` instances** in that PID (2 workers each). The TUI reuse path cannot do that unless thousands of `FileSearchState` daemons stay allocated (thousands of PromptWidgets that each used `@`) or `Nucleo` objects are created and never dropped.

The product map that can hold thousands of live matchers is `FuzzySearchManager.searches`. Each `open` without `close` adds two nucleo workers. Cleanup is only the next `open`, and only if that context has been idle 300 seconds. A client that opens a new search on every keystroke, every frame, or every `x.ai/search/fuzzy/status` and never closes will grow without bound until the next `open` after idle, or **forever** if opens stop (no periodic sweep). That matches a long-lived fat PID better than project picker, session list, history overlay, scratch, or slash MRU.

Dashboard `retarget` comments already treat daemon rebuild as expensive. That is one pool swap, not a storm. There is no residual or FORK note about nucleo threads (`RESIDUAL.md` / `FORK.md` have no nucleo/thread-storm pin).

## Most likely accumulation

Unclosed workspace `FuzzySearchContext` entries (ACP/hub `fuzzy_open` without `fuzzy_close`, 300s timeout, cleanup only on the next `open`, activity refresh on poll) each pin a 2-thread nucleo rayon pool. Secondary: never-pruned subagent `AgentView`s keep history-search threads (not nucleo) for the whole parent session.
