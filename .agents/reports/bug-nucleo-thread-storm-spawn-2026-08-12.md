# Nucleo thread storm: spawn sites and leak hypothesis

Observed: long-lived grok-oss processes grew thousands of OS threads named `nucleo` (about 3500 and 3300 in two PIDs). This report maps every spawn path in this tree and in the pinned helix nucleo crate (`git rev 5b74652`).

## Spawn sites

Only `nucleo::Nucleo::new` creates threads whose names start with `nucleo`. Helix names them `nucleo worker {i}` via rayon. `nucleo::Matcher` and `MultiPattern` are in-process and spawn nothing.

| Site | Type | Constructor | Threads spawned | Lifetime / Drop |
|---|---|---|---|---|
| `crates/codegen/xai-grok-workspace/src/file_system/fuzzy.rs` | `FuzzyFileMatcher` | `new` / `new_inner` calls `Nucleo::new(config, notify, Some(NUM_NUCLEO_THREADS), 1)` with `NUM_NUCLEO_THREADS = 2` | 2 rayon workers named `nucleo worker 0` and `nucleo worker 1` | `Drop for FuzzyFileMatcher` joins the ignore walk only. Then `Nucleo` drops. |
| Same file | `FuzzyFileMatcherDaemon` | `new` moves the matcher into `thread::Builder::name("fuzzy-daemon")` | 1 `fuzzy-daemon` thread (not nucleo) | `Drop` sends `Stop` and **joins** the daemon. Matcher (and Nucleo) then drop. |
| Same file | walk | `restart_walk_inner` | 1 `fuzzy-walk` plus up to 8 ignore-crate walk threads (`WalkBuilder.threads(NUM_IGNORE_THREADS)`) | `join_walk` cancels and joins `fuzzy-walk`. Ignore pool dies with the walk. |
| Same file | probe | `threads_spawnable` | Temporary `thread-probe` threads, joined before return | Not nucleo. Racy by comment. |
| `crates/codegen/xai-grok-workspace/src/file_system/mod.rs` | `FuzzySearchContext` | `new` always calls `FuzzyFileMatcher::new(root)` then `FuzzyFileMatcherDaemon::new` | A full matcher: 2 nucleo workers + daemon + walk | Lives in `FuzzySearchManager.searches` until `close` or `cleanup_stale`. |
| `crates/codegen/xai-grok-pager/src/views/file_search/state.rs` | `FileSearchState` | `ensure_daemon` (first `@` only) | Same as one `FuzzyFileMatcher` | `retarget` replaces the whole state (`*self = Self::new(root)`), which drops the daemon. Tests prove query edits reuse the daemon (`daemon_build_count` stays 1). |
| `crates/codegen/xai-grok-pager/src/slash/matcher.rs` | `slash::FuzzyMatcher` | `FuzzyMatcher::new` → `Matcher::new` + `MultiPattern::new` | **None** | Owned by `SlashController`. Command palette and `/` completion. |
| `crates/codegen/xai-grok-pager/src/views/history_search.rs` | `history_search::Daemon` | `Matcher::new` on a `history-search` thread | 1 `history-search` thread, **not** nucleo | `Drop` sends `Stop` and **does not join**. |
| `crates/codegen/xai-grok-shell/src/extensions/suggest/file_provider.rs` | shell file suggest | `nucleo::Matcher::new` per list | **None** | Sync ranker. |
| Session picker, project-style pickers, extensions modal, grep / content search | substring or ripgrep | no `Nucleo::new` | **None** | Not this storm. |

`Nucleo::new` exists in product code in **one** function: `FuzzyFileMatcher::new_inner`. rustc is not in this tree. Helix nucleo is a git dependency only.

Callers of `FuzzyFileMatcher::new`:

1. `FileSearchState::ensure_daemon` (TUI `@` completion: agent prompt, welcome, dashboard dispatch, dashboard peek reply).
2. `FuzzySearchContext::new` (workspace `FuzzySearchManager::open`, ACP `x.ai/search/fuzzy/open`, `workspace.fuzzy_open`).

## Pool sizing

Helix `Nucleo::new` (`~/.cargo/git/checkouts/nucleo-425d994cd74b3654/5b74652/src/lib.rs` and `src/worker.rs`):

- `num_threads: Some(n)` builds a **private rayon `ThreadPool`** with exactly `n` workers.
- `num_threads: None` uses `available_parallelism()` (this 16-thread Ryzen would get 16 workers per Nucleo). Documented on `Nucleo::new`.
- This tree always passes `Some(2)`. Cap is **per Nucleo instance**, not process-wide.
- Pool is **not** per keystroke and **not** per render tick. `tick` only `pool.spawn`s work onto the existing workers.
- `Nucleo::restart` keeps the same pool and replaces the item stream.
- `Drop for Nucleo` sets `canceled`, then `try_lock_for(1s)` on the worker. It does not name `join`. Rayon `ThreadPool` drop is what should stop the workers. If the lock times out, Drop hits `unreachable!`.

TUI `@` search: one Nucleo per `FileSearchState` after first `@`. Later keystrokes call `daemon.set_query` only.

Workspace fuzzy: one Nucleo **per `open`**, not per keystroke, unless the client opens a new search each time.

## Why count grows over hours

**Not** "one unbounded nucleo pool." Each pool is size 2.

**Not** TUI `@` recreating Nucleo per keystroke. `FileSearchState` is lazy and reused. Tests lock that.

**Yes: unbounded create of Nucleo instances that are not dropped.** About 3500 nucleo-named threads / 2 workers = about **1750 live `Nucleo` values** in one PID.

Best-supported leak:

`FuzzySearchManager::open` (`file_system/mod.rs`) always builds a new `FuzzySearchContext` (new Nucleo + daemon + walk) and inserts it in a `HashMap`. There is no process-wide cap. `cleanup_stale` runs **only on the next `open`**, with timeout **300s** on the workspace handle (`handle.rs`), not the 30s `Default`. `change`, `get_results`, and `get_results_filtered` all refresh `last_activity`. `fuzzy_poll` / `run_fuzzy_notifications` call those getters, so a polled search does not go stale while the 10s poll loop runs. If a client (ACP `x.ai/search/fuzzy/open`, hub `workspace.fuzzy_open`) opens a **new** search often and does not `close`, or if anything keeps polling so `last_activity` never expires, the map retains every Nucleo. That matches multi-hour growth and thousands of `nucleo worker` threads. RAM can stay fine: these are parked worker stacks, not a heap blowup.

Secondary TUI path (weaker for 3500 threads unless many composers live): each `PromptWidget` has its own `FileSearchState`. First `@` in that composer is a new Nucleo. Dashboard open retargets dispatch search (drops and later rebuilds). Peek reply retargets once per peeked cwd. Many live agents that have used `@` add 2 nucleo threads each. Closing a session does `shift_remove` and should Drop the daemon. This cannot explain thousands of threads unless thousands of `FileSearchState` daemons stay alive.

Weaker: Nucleo Drop fails to join (`unreachable` after 1s, or rayon pool left running). That would leak 2 threads per dropped matcher. Combined with frequent `retarget` / `open` it could grow. Evidence is weaker than the HashMap retain path.

Not this bug: slash command palette, session picker, history overlay, shell `Matcher` ranking, ripgrep content search.

## Suggested smallest product fix (do not implement here)

1. **Share one matcher per root (or per process).** Stop calling `Nucleo::new` in `FuzzySearchContext::new`. Reuse one `FuzzyFileMatcherDaemon` for that cwd. `open` should return an existing id or reset query, not spawn a second pool.
2. **Hard cap** `FuzzySearchManager.searches` (even `1` is enough). Evict oldest on insert. Run `cleanup_stale` on a timer, not only on `open`. Do not treat poll-only `get_results` as activity if that is what keeps dead searches alive.
3. **TUI:** one shared `FileSearchState` / daemon per cwd at `AppView`, not one Nucleo per `PromptWidget`. Keep the existing lazy first-`@` behavior.
4. Keep `Some(2)` (or `Some(1)`). Never pass `None` (that is one pool per hardware thread per instance).

Regression contract: after N `@` keystrokes and N `fuzzy_open` RPCs, `nucleo worker` thread count stays at a small constant (2 per live root, ideally 2 per process).
