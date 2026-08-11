# Plan inventory: rebuild grok-oss + graceful reboot of live instances

**Date:** 2026-08-07
**Scope:** Design / inventory only (no product implementation)
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Host skills checked:** `~/.agents/skills` (no rebuild/restart skill today)

---

## North star

After you change source and install a new `grok-oss` binary, every live Grok process on this host should pick up that binary the way a network blip is recovered: durable session state stays on disk, in-flight sampling/tool work is interrupted only as a cancel-or-reconnect would interrupt it, and sessions come back via existing load/resume paths (not a hard kill that leaves zombie chrome or invents work). Rebuild and reboot are one intentional operator action, not “recompile then manually kill every terminal.”

---

## What already exists

### 1. How grok-oss is built today

| Path | What it does | Binary location |
|------|----------------|-----------------|
| **Package** | Cargo package `xai-grok-pager-bin` (`default-run` / `[[bin]] name = "grok-oss"`) | `target/release/grok-oss` after release build |
| **`just install`** | `cargo build --release -p xai-grok-pager-bin --locked`, strip unneeded, install | `${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss` |
| **`just install-dist`** | `release-dist` profile + DWARF sidecar, then install binary + `grok-oss.debug` | same cargo bin dir |
| **`just install-nix`** | `just build` → `nix build .#grok-oss` → copy/strip | same cargo bin dir from `./result/bin/grok-oss` |
| **`just build` / `smoke`** | Nix release package only (not CI); smoke runs `--version` | `./result/bin/grok-oss` |
| **README cargo** | `cargo install --path crates/codegen/xai-grok-pager-bin --locked --force` | `~/.cargo/bin/grok-oss` |
| **AUR** | `packaging/aur/grok-oss-git` builds same package | `/usr/bin/grok-oss` |
| **Identity** | `build.rs` embeds `GROK_GIT_SHA`; version is lockstep package version + short SHA | shown by `--version` / `update --check` |

Notes:

- CI is **checks only** (`just check` / `just ci`); release package is human-local.
- Host `~/.cargo/config` wild linker is overridden in `just install*` recipes.
- Official SpaceXAI install (`curl … x.ai/cli/install.sh` → `grok`) is a **different product**; Grok OSS deliberately does not use that channel by default.

### 2. How multiple instances run

| Mechanism | Path / API | Role |
|-----------|------------|------|
| **Session store** | `$GROK_HOME/sessions/<encoded-cwd>/<session-id>/` (default `~/.grok`) | Durable transcript (`updates.jsonl`), todos, plan, cancel-resume marker, etc. |
| **Active session registry** | `$GROK_HOME/active_sessions.json` (+ flock lock) | Live TUI/headless: `session_id`, **pid**, `cwd`, `opened_at`. Clean exit unregisters; crash leaves orphan for `collect_crashed`. |
| **Leader–client IPC** | `$GROK_HOME/leader.sock` / `leader.lock` (or `GROK_LEADER_SOCKET` override) | One leader process holds agent state; multiple clients (TUI, IDE, headless) attach over Unix socket. |
| **Leader version eviction** | `leader_is_older_than` / `should_evict` | Newer client can replace an older leader (anti-thrash; converges to newest). |
| **Config / rate limits** | `$GROK_HOME/config.toml`, `$GROK_HOME/rate_limits/` | Shared across processes on the same host + `GROK_HOME`. |

Important: “all currently running instances” is **not** only “rows in `sessions/`.” Disk sessions can be cold. Live set ≈ **alive PIDs in `active_sessions.json`** plus any **reachable leaders** from discovery. Headless/IDE clients may share one leader; standalone TUIs register their own PID.

There is `list_in(root)` for active sessions (test/injectable root). Production callers use `register` / `unregister` / `collect_crashed`; a public `list()` convenience wrapper is thin to add if a tool needs it.

### 3. Graceful interrupt / resume contracts already in product

These are the behaviors the operator wants reboot to **resemble**, not invent from scratch.

| Contract | Where | What it does |
|----------|--------|--------------|
| **`RetryState::StreamResumed`** | shell `extensions/notification.rs`; pager ACP handler / turn status; shell stream-start emit | Soft-reconnect chrome after network/retry: clears sticky yellow Retrying, keeps attempt N, not zombie “Waiting for response…” across headers/TTFB. |
| **Stuck-retry / headers timeout** | FORK + sampler/shell | Stream headers/first-byte default 120s (`GROK_STREAM_HEADERS_TIMEOUT_SECS`); short transport footer labels. |
| **Cancel-aware cooldown** | `xai-grok-sampler` `request_task` wait on shared rate-limit store | Shared cooldown wait aborts early when cancel token fires (not stuck sleep through Esc). |
| **Identity / host failover** | sampler + dual-auth | Credit/429 hop to next identity; host may switch SuperGrok proxy ↔ `api.x.ai`. Mid-stream recovery is soft reconnect + new sample, not process restart. |
| **Fearless global pause** | `global_work_pause` + dispatch | `Ctrl+Shift+Space`: cancel turns in **this process**, hold queue drain, resume re-queues mid-turn once. **In-process only** (not cross-process). Does **not** write cancel-resume marker. |
| **Soft stop** | `soft_stop` | `Ctrl+Shift+S`: finish current top-level turn, then hold queue (no mid-flight cancel). |
| **Resume canceled turn on restart** | `canceled_turn_resume.json` + session load | Explicit Esc/stop persists prompt; on reopen, if `[ui] resume_canceled_turn_on_restart` (default on), re-queue **once** + toast. Not for clean success / pause / soft stop. |
| **Leader `RelaunchForUpdate`** | `leader/protocol.rs` `ControlCommand::RelaunchForUpdate` | Leader stops new turns, **bounded grace** for in-flight, flush, exit `ShutdownReason::AutoUpdate`; clients reconnect and **`session/load`**. Capability flag `relaunch_v1`. |
| **`signal_leaders_to_relaunch`** | `xai-grok-pager-bin` `main.rs` | After successful **xAI** `grok update` install path: discover leaders, send `RelaunchForUpdate`. Best-effort. |
| **`restart_grok` / screen-mode re-exec** | `auto_update.rs` `restart_grok`; pager `screen_mode_relaunch` | Unix `exec` same argv (update restart) or re-exec for `/minimal` ↔ fullscreen. Process image replaced; PTY can stay. |
| **Quit-for-update (Ctrl+U)** | pager event loop + `finish_update_on_exit` | Upstream-oriented: quit TUI, finish download, print “run `grok` again.” **Grok OSS disables background xAI auto-update** unless `GROK_OSS_ENABLE_XAI_UPDATER=1`. |
| **OSS update UX** | `oss_update.rs` + `run_update_command` | Default: `grok-oss update --check` (SHA vs Surmount `main`); plain `update` prints rebuild recipe (`just install` / Nix / AUR). **No binary download, no multi-instance reboot.** |

Regression filters (StreamResumed etc.) live in `doc/dev/upstream-regression-filters.md` and FORK.

### 4. Existing skill / slash / tool for rebuild, install, upgrade, restart

| Surface | Rebuild from source? | Restart all live instances? |
|---------|----------------------|-----------------------------|
| Host skills under `~/.agents/skills` | No dedicated skill (git-recon “rebuild” = onto stack, not product binary) | No |
| Product slash / tools | No “rebuild” tool | No multi-instance reboot tool |
| `grok-oss update` | Message only (OSS) | Only if xAI updater env + install succeeds → leader relaunch |
| `just install*` | Yes (human shell) | No |
| User-guide install docs | Yes | Manual relaunch |
| resume-claude / resume-codex / resume-cursor | Resume **other** products’ sessions | N/A |

**Gap:** nothing today is “one action: rebuild this tree’s `grok-oss` and gracefully bounce every live process on this host.”

### 5. What “all currently running instances” can mean safely

Safe default for v1:

- **Same OS user**, same default or explicit **`GROK_HOME`** (do not touch other users’ homes).
- **Same host** (Unix sockets + PID checks are local).
- Live set from:
  1. `discover_leaders()` → reachable leaders with `relaunch_v1` → `RelaunchForUpdate`.
  2. `active_sessions` entries whose **PID is alive** and whose executable looks like `grok-oss` / product CLI (avoid signaling random PIDs if the registry is stale).
- Optional filter: only processes whose `/proc/<pid>/exe` resolves to the path you just installed (or same basenames under cargo bin / nix result).

Out of safe scope without explicit operator OK:

- Other machines, containers without shared `GROK_HOME`/sockets, other users.
- Official `grok` (SpaceXAI) processes.
- Zombie registry rows with dead PIDs (clean via `collect_crashed`, do not “resume invent”).
- Killing processes that only share the string “grok” in argv.

---

## Recommended shape: skill vs product vs both

### Recommendation (phased)

**Both**, with a clear vertical split. Skill-only cannot fully match “network-class resume” for the **process that is running the skill** (you cannot `exec` yourself out of an in-flight turn without product cooperation). Product already owns leader relaunch and session load; OSS just never wires rebuild → that path.

#### Phase A — v1 smallest useful (ship value fast)

1. **Host skill** (e.g. `rebuild-grok-oss` under `~/.agents/skills/` or project skill if you want it in-repo):
   - Resolve repo root (this tree or operator-passed).
   - Run preferred install: default **`just install`** (or `just install-dist` / `install-nix` / bare cargo when `just` missing).
   - Verify `${CARGO_HOME}/bin/grok-oss --version` (or installed path).
   - **Signal leaders** if possible: either call a small product CLI once it exists, or for A0 document that operator runs a temporary script using the same control protocol as `signal_leaders_to_relaunch`.
   - List live `active_sessions` (alive PIDs) and report: which leaders relaunched, which **standalone TUI PIDs** still need operator reattach (`grok-oss --resume <id>` or open from picker).
   - **Do not** hard-kill mid-tool by default.

2. **Thin product CLI** (preferred even for v1 if effort allows): e.g.
   `grok-oss install-from-source` is optional; more important is
   **`grok-oss relaunch-running --to-version <ver-or-sha>`** (or reuse/control-only extract of `signal_leaders_to_relaunch` without xAI download).
   That reuses `discover_leaders` + `RelaunchForUpdate` without enabling SpaceXAI GCS.

**Skill alone for v1** is acceptable if product CLI is deferred: skill runs `just install`, then uses `ps`/active_sessions for reporting, and only soft-signals leaders via a one-off `cargo run -p xai-grok-pager-bin -- …` once a control subcommand exists. Without any control path, skill can only rebuild + print “restart these PIDs / use resume,” which is weaker than the north star but still useful for dogfood.

#### Phase B — product-grade multi-instance reboot

- **Source rebuild path** on product: `grok-oss rebuild` (or skill stays the only builder) + **always** `signal_leaders_to_relaunch` after install using **package version + git SHA**, not xAI channel.
- **Standalone TUI grace path** (no leader):
  - Option 1: write a cooperative “please quit for upgrade” marker under `$GROK_HOME` that the event loop watches (toast + soft stop arm + quit-for-update).
  - Option 2: send a signal the product already handles (if any); today there is no dedicated SIGUSR for graceful TUI quit.
  - Option 3: on restart only, rely on **canceled_turn_resume** if the process was canceled, or plain **session/load** if exit was clean after soft stop.
- Align **self-process** (the TUI that invoked rebuild): like Ctrl+U quit-for-update, then re-exec installed path with same session id (pattern exists in `screen_mode_relaunch` + `restart_grok`).
- Version guard: leaders already decline if not older; OSS should compare **semver package + SHA**, not xAI channel pointers.

#### Phase C — “network-call class” fidelity (fancy)

- Mid-turn: prefer **soft stop or pause-class cancel** + durable cancel marker before process exit (so reopen re-queues once).
- Mid-tool: treat like transport cancel (tool error path), not undefined half-write; no new invent of tool success.
- Mid-plan-approval: soft-park state is on disk (`plan.md`); process death should leave panel restorable on load (verify; add test if gap).
- Shared rate-limit / exhausted-credit memos already on disk under `GROK_HOME`; survive reboot.
- Optional: broadcast “binary replaced” to all clients so chrome shows reconnecting (`StreamResumed`-like) during leader grace.

**Do not** make the primary path “skill runs `kill -9` on every grok-oss PID.” That fights every resume contract above.

---

## Acceptance criteria

### v1 (smallest useful)

1. From a known checkout, one documented action (skill or CLI + skill) produces a **new** installed `grok-oss` at the intended path and prints version/SHA.
2. **Reachable leaders** older than the installed identity receive `RelaunchForUpdate` (or equivalent) and exit with auto-update/relaunch semantics; connected clients can reattach via existing reconnect + `session/load`.
3. Skill/CLI reports **live active_sessions** (alive PIDs, session ids, cwds) and clearly separates: relaunched via leader vs still need manual reattach.
4. Default path does **not** enable `GROK_OSS_ENABLE_XAI_UPDATER` or install SpaceXAI binaries.
5. Failure modes are loud: build fail stops before signaling; connect fail to a leader is best-effort skip with log, not silent success.
6. No requirement yet for zero-operator-touch reboot of every standalone TUI.

### Fancy full resume (later)

1. All live product processes on this user/`GROK_HOME` either relaunch in-place (`exec`) or exit after grace with durable state.
2. In-flight user turn either completes (soft stop) or is canceled with **`canceled_turn_resume`** so reopen re-queues once (same toast contract).
3. No invented finished work; no resume of success turns.
4. Mid-plan-approval and mid-tool recover without data loss beyond normal cancel.
5. Chrome on reconnect matches StreamResumed / reconnecting language, not stuck Retrying.
6. Tests: leader relaunch after local install; standalone active_session + marker; binary-replace-while-running does not corrupt install (atomic `install` already used).
7. Multi-session same process: all sessions in that process reload (leader or multi-agent pager), not only focused one.

---

## Risks

| Risk | Why it matters | Mitigation |
|------|----------------|------------|
| **Mid-tool** | Hard kill can leave half-applied edits, open PTYs, orphan subagents | Prefer leader grace drain; soft stop before exit; cancel markers only on explicit cancel; never claim tool success after reboot |
| **Mid-plan-approval** | Soft-park / CTA state must survive process death | Rely on disk `plan.md` + session hydrate; add regression test before claiming fancy |
| **Multi-session / multi-process** | Pause is in-process; leader relaunch is cross-client; standalone TUI is orphan | Inventory both leaders and active_sessions; do not assume one PID = one session |
| **Binary replace while running** | Linux keeps old inode mapped until exec/exit; new clients get new binary, old processes stay on old code | Explicit relaunch required; do not claim “install alone upgrades live memory” |
| **Wrong binary path** | AUR `/usr/bin`, cargo bin, nix result, and `current_exe` may differ | Resolve install target; match `/proc/pid/exe`; document which path skill updates |
| **Leader thrash** | Newer/older version guards exist for a reason | Keep directional `leader_is_older_than`; decline same/newer |
| **Self-rebuild from inside TUI** | Skill running in the old process cannot finish after `exec` of self without product quit path | Phase B self quit-for-rebuild; v1 allow “other terminals bounce, this one restart manually” |
| **xAI updater confusion** | Opt-in env installs wrong product | Keep OSS gate; never wire skill to SpaceXAI channel |
| **Stale active_sessions** | Dead PIDs or recycled PIDs | `is_pid_alive` + exe basename check; `collect_crashed` first |
| **Cross-user / shared machine** | Signaling another user’s leader is hostile | Only own `GROK_HOME` and own UIDs |

---

## Open operator questions (few, high-signal)

**Q1.** Should v1 only update **`just install` → `~/.cargo/bin/grok-oss`**, or must it also support **Nix (`install-nix`)** and **AUR `/usr/bin`** in the first cut?

**Q2.** For **standalone TUIs** (no leader), is it acceptable that v1 **rebuilds + reports PIDs** and only **auto-relaunches leaders**, with full TUI self-restart deferred? Or is zero-touch every TUI required on day one?

**Q3.** When a turn is mid-flight at reboot, prefer **soft-stop finish then exit**, **cancel + canceled_turn_resume**, or **leader grace only (no cancel marker)**?

**Q4.** Should the primary entry be a **host skill** agents call, a **product CLI** humans and skills both run, or **both from the start** (recommended above)?

**Q5.** Scope of “all instances”: **this `GROK_HOME` only**, or also discover alternate homes / `GROK_LEADER_SOCKET` sandboxes?

---

## Suggested implementation order (when approved)

1. Document operator recipe (already partly in `oss_update` how-to).
2. Product: extract/publish **relaunch-running leaders** without xAI download (copy of `signal_leaders_to_relaunch` + discover).
3. Skill: `just install` → verify → call relaunch CLI → print active_sessions report.
4. Tests around control command + discovery with fake leaders.
5. Phase B: cooperative TUI quit-for-rebuild + re-exec with session id; cancel-resume on forced cancel path.
6. Fancy chrome + mid-plan tests only after v1 is dogfooded.

---

## Critical files for implementation

- `justfile` — install recipes and target binary path
- `crates/codegen/xai-grok-pager-bin/src/main.rs` — `signal_leaders_to_relaunch`, OSS update gate, `run_update_command`
- `crates/codegen/xai-grok-shell/src/leader/protocol.rs` (+ server handler) — `RelaunchForUpdate`, grace, `ShutdownReason::AutoUpdate`
- `crates/codegen/xai-grok-shell/src/active_sessions.rs` — live PID inventory
- `crates/codegen/xai-grok-shell/src/session/canceled_turn_resume.rs` + pager session load — restart-after-cancel resume
- `crates/codegen/xai-grok-update/src/oss_update.rs` + `auto_update.rs` (`restart_grok`) — OSS messaging and re-exec patterns
- `crates/codegen/xai-grok-pager/src/app/screen_mode_relaunch.rs` — same-process re-exec with session argv

(Host skill, when added: `~/.agents/skills/<name>/SKILL.md` following `create-skill` / `_SKILL_RULES`.)

---

## Summary table: resume contracts vs invent

| Need | Exists? | Reuse for reboot? |
|------|---------|-------------------|
| Soft network reconnect chrome | Yes (`StreamResumed`) | Yes for client reconnect UX |
| Cancel-aware waits | Yes | Yes if cancel during drain |
| Durable session reload | Yes (`session/load`, updates.jsonl) | Yes primary recovery |
| Cancel-then-reopen prompt once | Yes (`canceled_turn_resume`) | Yes if reboot cancels mid-turn |
| In-process pause/resume all agents | Yes (global pause) | No cross-process; wrong tool for binary upgrade |
| Leader multi-client relaunch after new binary | Yes (`RelaunchForUpdate`) | **Yes core** (wire without xAI install) |
| Rebuild from this git tree | Yes (`just install*`) | Yes skill/CLI wrapper |
| One-shot “rebuild + bounce all” | **No** | To build |

---

*End of inventory. No product code changed in this pass.*
