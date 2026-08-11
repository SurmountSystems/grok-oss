# Plan: product slash `/rebuild` (graceful rebuild + relaunch)

ASCII only. Plain American English. Complete sentences.

**Date:** 2026-08-07
**Intent:** One operator action, the builtin slash **`/rebuild`**, rebuilds
`grok-oss` from this tree and gracefully reboots live instances so durable
session state survives and in-flight work is interrupted only the way a
network cancel / soft-reconnect would interrupt it. Not hard `kill -9`. Not
the SpaceXAI auto-updater channel.

**Prior inventory (read, do not re-litigate):**
- `.agents/reports/plan-rebuild-reboot-graceful-2026-08-07.md`
- `.agents/reports/explore-rebuild-slash-seams-2026-08-07.md`

## Context

### Why

Dogfood today is rebuild-by-hand, then manual restart of every TUI. Product
already has leader `RelaunchForUpdate`, session load, StreamResumed, and
canceled-turn-on-restart. OSS never wires local rebuild to those paths.
`/rebuild` is the name the operator wants.

### Constraints

- Same OS user and same `GROK_HOME` only (default `~/.grok`).
- Default install path: **`just install`** →
  `${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss`.
- Do **not** enable `GROK_OSS_ENABLE_XAI_UPDATER` or download SpaceXAI binaries.
- Builtin slash (not agent freeform shell): fixed argv build, not arbitrary
  shell from the model.
- Multi-minute cargo must be async (`Action` / `Effect` / `JoinSet` /
  `TaskResult`), never block the slash `run()` thread.
- Agents never `git commit`; this feature is product code only.
- Complete plan verticals: ship a closed useful `/rebuild` vertical, not a
  half-skill that only prints PIDs.

### Non-goals (v1)

- Nix (`install-nix`) and AUR as first-class install backends (document
  later; optional flag is fine if free).
- Cross-user or remote hosts.
- Perfect mid-tool invent-success recovery (cancel paths already exist).
- Host-only skill as the **primary** entry (skill may wrap the same CLI
  later; product `/rebuild` is the SoT operator UX).
- Worktree DB rebuild (different RPC; help text must disambiguate).

### Assumptions (operator locked in chat)

1. Primary name and entry: **`/rebuild`**.
2. Behavior: rebuild + **graceful relaunch** of live instances (network-class
   interrupt / resume), not rebuild-only report.
3. The **invoking TUI** must come up on the new binary with the same session
   when possible (self re-exec after success), not only bounce other leaders.
4. Default source tree: walk from process cwd (and optional project roots) to
   a checkout that has this repo's `justfile` install recipe; fail loud if
   not found (toast / scrollback). Override later if needed.

## Approach

### North star

`/rebuild` means:

1. Resolve source checkout.
2. Build and install a new `grok-oss` (`just install` equivalent, fixed argv).
3. Verify installed binary `--version` (package version + git SHA).
4. Soft-signal **reachable leaders** to drain and exit for upgrade
   (`RelaunchForUpdate` / existing grace).
5. Soft-exit **this process** after durable cancel/resume arming when a turn
   is mid-flight, then **re-exec** the newly installed binary into the same
   session (same class as screen-mode relaunch / update restart).
6. Report what relaunched vs still needs reattach (standalone clients that
   were not leaders and did not self-exec).

Resume class matches existing contracts:

| Situation | Behavior |
|-----------|----------|
| Leader with clients | Grace drain (~5s idle + flush), `ShutdownReason::AutoUpdate`, clients reconnect + `session/load` |
| Mid-turn in this TUI | Prefer cancel + `canceled_turn_resume` (re-queue once on restart) so reopen is network-like, not invent success |
| Mid-plan-approval | Disk `plan.md` survives; panel reopens from durable park on load |
| Idle TUI | Clean re-exec, session load |
| Shared rate-limit / exhausted memos | Already under `GROK_HOME`; survive |

### Critical product fix: version identity for local rebuilds

Today `leader_is_older_than` / `decide_relaunch_for_update` require a
**strictly greater parseable semver**. Same package version + new git SHA
**declines** relaunch. Local dogfood rebuilds almost always keep the same
crate version and only change SHA.

**Required for `/rebuild` to work:** relaunch identity must treat **package
version + git SHA** (or full binary identity already embedded by `build.rs`)
so a newer SHA with the same semver is "newer" for local rebuild. Keep
xAI-channel install path semantics stable if they depend on semver only;
gate the SHA-aware comparison behind local-rebuild / OSS identity, or
generalize carefully with tests.

Without this, `/rebuild` installs a new binary and leaders silently stay on
the old image.

### Architecture (one vertical)

```
/rebuild (slash)
  → Action::RebuildAndRelaunch
  → Effect (async):
       resolve_source_root
       run just install (or cargo equivalent fixed argv)
       verify installed path --version
       signal leaders (shared helper, not xAI-only)
       inventory active_sessions (alive PIDs)
       TaskResult::RebuildDone { report }
  → UI: scrollback report + toast
  → self: arm cancel-resume if needed, re-exec installed grok-oss with same session
```

Also expose a thin CLI for agents and scripts:

`grok-oss rebuild` (and/or `grok-oss relaunch-running`) so the same code path
runs outside the TUI. Slash is the human dogfood entry; CLI is the same core.

### Not these

- **Not host-skill-only** as primary: cannot self-re-exec the TUI that ran the
  skill with full product grace without product cooperation.
- **Not hard kill** of every `grok` PID.
- **Not** enabling SpaceXAI updater.
- **Not** inventing freeform "type rebuild in chat" menus.

## Critical files

| Path | Why |
|------|-----|
| `crates/codegen/xai-grok-pager/src/slash/commands/rebuild.rs` (new) | Builtin `/rebuild` |
| `…/slash/commands/mod.rs` | Register in `builtin_commands()` |
| `…/slash/command.rs` + dispatch / effects / task_result | Action / Effect / TaskResult wiring |
| `crates/codegen/xai-grok-pager-bin/src/main.rs` | Factor `signal_leaders_to_relaunch`; add `rebuild` CLI; install/relaunch entry |
| `crates/codegen/xai-grok-shell/src/leader/protocol.rs` | `RelaunchForUpdate` |
| `…/leader/server.rs` | `decide_relaunch_for_update`, grace drain |
| `…/leader/mod.rs` | `leader_is_older_than`, `discover_leaders` (**SHA-aware identity**) |
| `…/active_sessions.rs` | Live PID inventory; thin `list()` if missing |
| `…/session/canceled_turn_resume.rs` | Mid-turn reboot resume |
| `crates/codegen/xai-grok-pager/src/app/screen_mode_relaunch.rs` | Pattern for same-session re-exec |
| `crates/codegen/xai-grok-update/src/auto_update.rs` | `restart_grok` re-exec pattern |
| `crates/codegen/xai-grok-update/src/oss_update.rs` | Point "how to update" at `/rebuild` |
| `justfile` | `install` recipe (call, do not rewrite unless needed) |
| User-guide slash / install docs | Document `/rebuild` |
| `FORK.md` | Short lasting bullet when shipped |

## Reuse

| Symbol | How |
|--------|-----|
| `signal_leaders_to_relaunch` | Extract to shared shell/update helper; call from slash + CLI |
| `discover_leaders` + `RelaunchForUpdate` | Core multi-client bounce |
| `list_in` / `collect_crashed` | Inventory + stale PID hygiene |
| Doctor Effect / TaskResult | Template for multi-minute host work + scrollback |
| `restart_grok` / screen-mode re-exec | Self re-exec after install |
| `canceled_turn_resume` | Mid-turn interrupt class |
| `GROK_GIT_SHA` / `--version` | Post-install verify + identity compare |

## Steps

Ordered; complete vertical unless operator defers a step explicitly.

### 1. Identity: same semver + newer git SHA can relaunch

- Named pure helpers: "is this installed binary newer than that running
  leader identity for local rebuild?"
- TDD red → green on leader decide path (semver equal, SHA newer → accept;
  SHA equal or older → decline; unparseable stay safe).
- Do not break intentional "do not thrash to older" guards.

### 2. Shared rebuild-and-relaunch core (library + CLI)

- Resolve source root (cwd walk for justfile + `xai-grok-pager-bin` package).
- Run install: prefer `just install` when `just` exists; else fixed
  `cargo build --release -p xai-grok-pager-bin --locked` + install to cargo
  bin (match justfile behavior).
- On build fail: stop before any relaunch signal; loud error.
- On success: verify target binary version/SHA.
- `collect_crashed` optional hygiene; list alive active sessions.
- `signal_leaders_to_relaunch` with new identity (best-effort per leader;
  report skips).
- CLI: `grok-oss rebuild` runs the same core (for agents / scripts / skill
  later).

### 3. Builtin slash `/rebuild`

- New command module; help text: rebuild this tree's binary and gracefully
  relaunch live Grok instances (not worktree DB rebuild).
- Async Effect: progress toast / scrollback lines (started, building,
  installed version, leaders signaled, self relaunching).
- After success on the invoking TUI: arm cancel-resume if a turn is running,
  then re-exec installed path with session continuity.
- Register in `builtin_commands()`; name free (confirmed).

### 4. Operator report + docs

- Scrollback summary: installed path, version, leaders relaunched, other
  live PIDs still running old binary (if any), how to reattach.
- User-guide: slash command + install chapter pointer.
- `oss_update` how-to message mentions `/rebuild`.
- FORK short bullet when green.

### 5. Fancy fidelity polish (same campaign if time; not optional-feeling park)

Still in scope unless dogfood proves deferred:

- Standalone **other** TUIs (not leaders, not self): cooperative marker or
  best-effort report only for v1 if product has no SIGUSR; do **not** claim
  zero-touch for them without a real mechanism. Prefer: document + report
  in v1, add cooperative quit marker if a small seam exists.
- Chrome language on reconnect: reuse StreamResumed / reconnect wording,
  not sticky Retrying.
- Regression: mid-plan-approval disk survival (session load restores park).

### 6. Verify + mop

- Targeted unit tests for identity + slash registration + pure resolve.
- Hermetic leader decide tests (no live cargo install in unit tests).
- Optional integration: mock install success path without full release
  build in CI (CI is checks only; do not require full `just install` in GHA).
- `cargo fmt` + clippy on touched packages; filters for new tests.

## Risks

| Risk | Mitigation |
|------|------------|
| Semver-only relaunch no-op | Step 1 is blocking; ship with SHA-aware identity |
| Multi-minute build freezes TUI | Async Effect only; progress messages |
| Mid-tool half-write | Cancel/resume; never claim tool success after reboot |
| Wrong source tree | Fail if justfile/package not found; show resolved path |
| Binary replace while process maps old inode | Explicit relaunch/re-exec; never claim install alone upgrades live memory |
| Self-re-exec fails | Leave clear toast + "run grok-oss again"; binary still installed |
| Stale active_sessions PIDs | Alive check + exe basename; collect_crashed |
| Confuse with worktree rebuild | Help + user-guide wording |

## Verification

### Red → green (named contracts)

1. **Local-rebuild identity:** leader with same package semver and older git
   SHA accepts relaunch to newer SHA; equal SHA declines.
2. **Slash exists:** `/rebuild` is registered (help list / unit registry).
3. **Build fail does not signal leaders:** pure or hermetic mock of core
   orchestration order.
4. **Success path calls relaunch helper** with new version identity (mock
   discover/signal).
5. Broader: existing relaunch / canceled_turn_resume / StreamResumed filters
   still green where touched.

### Manual dogfood (operator)

1. Two sessions on same host if possible.
2. `/rebuild` from one; watch install progress; confirm new `--version` SHA.
3. Leaders bounce; clients reload session.
4. Invoking TUI reappears on same session; mid-turn either finished or
   re-queued once with existing resume toast.
5. Compact status / limits still honest after restart (no invented meters).

## Open questions (non-blocking defaults)

Defaults locked unless you revise:

- **Install backend:** `just install` / cargo bin first. Nix/AUR later.
- **Mid-turn policy:** cancel + `canceled_turn_resume` for the invoking
  process; leader grace for multi-client leaders.
- **Other standalone TUIs:** v1 report + optional reattach instructions;
  self + leaders are auto. Cooperative marker if small; do not kill -9.
- **Scope:** this `GROK_HOME` only.

If you want Nix install or hard zero-touch every standalone TUI on day one,
use **Revise** on the plan panel.

## Critical Files for Implementation

- `crates/codegen/xai-grok-pager/src/slash/commands/rebuild.rs` (new)
- `crates/codegen/xai-grok-pager/src/slash/commands/mod.rs`
- `crates/codegen/xai-grok-shell/src/leader/mod.rs` (+ server decide path)
- `crates/codegen/xai-grok-pager-bin/src/main.rs`
- `crates/codegen/xai-grok-shell/src/active_sessions.rs`
- `crates/codegen/xai-grok-pager` effects / task_result / dispatch
- `crates/codegen/xai-grok-update/src/oss_update.rs` + re-exec helpers
- `justfile` (call site only)
- User-guide slash docs + `FORK.md` when shipped

## References on disk

- `.agents/reports/plan-rebuild-reboot-graceful-2026-08-07.md`
- `.agents/reports/explore-rebuild-slash-seams-2026-08-07.md`
