# Session Management

Grok saves every conversation to disk automatically. Whether you work in the TUI, in headless mode, or over agent stdio, Grok records the exchange as a session. You can resume, rewind, or compact it. This document describes how to manage sessions.

---

## What Sessions Are

A session is a persistent conversation with full history. It includes:

- All user prompts and agent responses
- Tool calls and their results
- TODO/task list state
- Rewind points for undoing later turns
- Token usage and turn counts
- Subagent sessions (when enabled)

Sessions are identified by a unique session ID (a UUIDv7 when Grok generates it; a client may supply its own ID with `-s`) and stored on disk under `~/.grok/sessions/`. Set `GROK_HOME` to override the base directory; when it is unset, Grok uses `~/.grok`.

---

## Storage Layout

Grok stores each session in its own directory, grouped by working directory. It URL-encodes the working directory to name the group. When the encoded name exceeds 255 bytes, it instead uses a slug plus a hash and records the original path in a `.cwd` file inside the group.

```
~/.grok/sessions/<encoded-cwd>/<session-id>/
  summary.json            # metadata: summary/title, timestamps, model ID, message counts
  updates.jsonl           # ACP session update stream (conversation + tool calls)
  chat_history.jsonl      # raw chat messages sent to the model
  system_prompt.txt       # the rendered system prompt, as sent to the model
  prompt_context.json     # the inputs the system prompt was rendered from
  tool_definitions.json   # function tools sent on the latest model call (no MCP server__tool entries)
  plan.json               # TODO/task list state
  rewind_points.jsonl     # rewind points for /rewind undo
  signals.json            # session signals (token usage, tool/turn counters)
  feedback.jsonl          # user feedback and ratings
  compaction_checkpoints/ # saved state from compaction (manual or auto)
  subagents/              # per-subagent metadata (meta.json); the child sessions live in the normal sessions tree
  canceled_turn_resume.json  # optional: mid-turn cancel/quit marker for auto-continue
  unsent_prompt_draft        # composer text that has not been sent
  pending_prompts.json       # local pager queue
  prompt_wal.jsonl           # append-only operator prompt write-ahead log
```

`summary.json` is the index entry. It records the session summary and generated title, the model ID, the creation and update timestamps, the message counts, and a parent session reference for forked or restored sessions. It also records the latest last-turn summary and session recap so listing surfaces can show them. `updates.jsonl` is the authoritative conversation log that drives `/resume` and session restore. `tool_definitions.json` omits MCP `server__tool` entries because the model reaches those through `search_tool` and `use_tool`, which are listed. Per-turn token and cost totals are available through `grok usage`.

### Session titles

The session title shown in the dashboard and `/resume` is generated automatically from the conversation. The prompt border shows a title only after a manual `/rename`, alongside the `Stashed` caption when a draft is stashed. Title generation starts right after your first prompt so a session always has a title, and then the title is regenerated from the whole conversation at a couple of early turns and frozen. This lets the title move past a vague first prompt to reflect what the session is really about, while staying stable afterward so you don't lose track of your sessions. A manual `/rename` always wins: once you rename a session, automatic generation never overrides it. Use `/rename --auto` to hand the title back to automatic generation.

When a mid-turn is interrupted in a cancel-resumable way, Grok OSS may write `canceled_turn_resume.json` with the in-flight prompt identity (not secrets). On the next open of that same session, if **`[ui] resume_canceled_turn_on_restart`** is on (default **true**, Settings → Session → **Continue interrupted turn on restart**), Grok OSS re-queues that prompt once and clears the marker.

**Writes the marker:** explicit cancel (`Esc` / `[stop]`), graceful quit while a turn is running, `/rebuild` mid-turn before self re-exec, and fearless global pause when it cancels a running turn (`Ctrl+Shift+Space`, status `[pause]` / `[resume]` when painted).

**Does not write a durable cancel-resume marker:**

- Clean success (a successful finish clears any leftover marker)
- Global pause when nothing is mid-turn. The pause gate itself stays in this process in RAM; only a canceled running prompt writes `canceled_turn_resume.json`
- Soft stop (`Ctrl+Shift+S` only). That holds the queue after the current turn
- `SIGKILL` before any turn-start write

Do not confuse these:

| What | What it does |
|------|----------------|
| Last session for this cwd | Bare `grok-oss` opens that session. Not the Welcome picker. |
| Continue interrupted turn | `canceled_turn_resume.json` plus the restart setting. |
| `/resume` or `--resume` | You pick a session (or continue the most recent globally, per CLI). |
| `/start` | Starts paused or interrupted work in the current session. Not the picker. |
| `/unstick` | Resend the last parent prompt as if the network dropped it. Orphans a hung in-flight prompt. The leader drops that hung `session/prompt` the same way as a disconnected client. WAL images resend as resource links, never data URLs. Not `/resume`. Not a second Human line. |
| Running grok-oss sessions | `/running` (alias `/windows`) or `grok-oss running`. Live grok-oss TUI windows on this machine. Not the Agent Dashboard, and not disk history. |

`summary.json` is the index entry. It records the session summary and generated title, the model ID, the creation and update timestamps, the message counts, and a parent session reference for forked or restored sessions. `updates.jsonl` is the authoritative conversation log that drives `/resume` and session restore.

---

## Starting and Ending Sessions

### New Session

Bare `grok-oss` does **not** start a new session when this working directory already has one. See [From the Welcome Screen](#from-the-welcome-screen). To start fresh mid-session:

```
/new
```

This clears the current context and begins a new conversation. Alias: `/clear`.

### Exit

End the session and quit Grok:

```
/quit
```

Alias: `/exit`. To leave the current session but stay in Grok, use `/home` to return to the welcome screen.

### Delete the current session

```
/delete
```

Confirms, then permanently removes the session history. Returns to the welcome screen, or to the dashboard when you opened the session from the dashboard. From `/resume` or the welcome session list, press `d` then `y`. On the [Agent Dashboard](23-dashboard.md), `Ctrl+X` twice (or hover `[✗]`) permanently deletes.

---

## Resuming Sessions

### From the TUI

Use the `/resume` command to browse and resume previous sessions:

```
/resume
```

This opens a session picker that lists recent sessions for the current workspace. Select a session to resume it. The command takes no arguments.

Typing in the picker filters the list by title and also searches your conversation content as you type; content matches appear under an "Extended search results" heading. Press `Ctrl+/` to search immediately without the brief pause.

For the live top-level sessions in this pager (parent and forks) — switch, rename, peek, dispatch, or close — use the [Agent Dashboard](23-dashboard.md): `/dashboard` (aliases `/sessions`, `/agents-dashboard`) or `Ctrl+\`.

### From the Command Line

Resume a specific session by ID or title:

```bash
grok-oss --resume <session-id-or-title>
```

A value that is not a session ID is matched against session titles for the current directory, ignoring letter case (a simple lowercase comparison) — handy after `/rename`. If several sessions share the title, a single manually renamed session wins over auto-generated duplicates; otherwise the command errors and lists the matching IDs. UUID-shaped values are always treated as session IDs, never titles. Scripts should prefer IDs.

Run `grok-oss --resume` without a value to resume the most recent session for the current directory.

### From the Welcome Screen

When you launch bare `grok-oss` (no `--resume`, `--continue`, or `--session-id`) and this directory already has a session, **that last session opens**. You do **not** land on the Welcome session picker first. First-ever use, or no last session here, shows the welcome screen with recent sessions. Select one to resume it.

This is **not** continue interrupted turn (`canceled_turn_resume.json`). That is a different feature: see [Continue interrupted turn on restart](#continue-interrupted-turn-on-restart). It is also not the `/resume` picker. Headless (`-p`) still starts a fresh session unless you pass `-c` / `--continue` or `--resume`.

The last-session pointer lives at `~/.grok/projects/<workspace-hash>/last_session`. If that file is missing, empty, or points at a session that is gone, you get a new session.

---

## Forking and Renaming Sessions

### Fork

Branch the current session into a peer agent that starts from a copy of the conversation:

```
/fork [--worktree|--no-worktree] [directive]
```

Pass an optional `directive` to set the new session's first prompt. Use `--worktree` or `--no-worktree` to choose whether the fork runs in a new git worktree; omit both to be asked each time. The `--at <turn>` flag is not supported in this version.

### Rename

Rename the current session's title:

```
/rename <title>
/rename --auto
```

Alias: `/title`. `/rename --auto` clears a manual title and re-enables auto-titling.

---

## The /rewind Command

`/rewind` (alias `/undo`) rewinds the conversation to an earlier turn, dropping later turns. File changes made after that turn are left as-is on disk.

```
/rewind
/undo
```

When you run `/rewind` or `/undo` (or press **Esc Esc** within 800ms while idle with an empty prompt and conversation messages), Grok:

1. Shows a list of rewind points (one per user prompt)
2. Lets you select which point to rewind to
3. Truncates the conversation history to that point

When **Confirm before rewind** is on (default in `/settings`), every pick asks for confirmation (Yes / Yes, and don't ask again / No). **Yes, and don't ask again** turns that setting off. With the setting off, picks run immediately.

**Important:** `/rewind` does not restore files on disk. Only conversation history is truncated.

---

## The /compact Command

`/compact` compresses the conversation history to save context window space. Use it in long sessions where early messages are no longer relevant.

```
/compact
```

### Auto-Compact

Grok automatically compacts the conversation when the context window approaches its limit. You will see a notification when auto-compact triggers. The `context_window` setting on your model configuration controls when this threshold is reached.

---

## The /session-info Command

View details about the current session:

```
/session-info
```

This shows:

- Session title (when set)
- Shell version
- Auth method (OAuth vs API key; API-key sessions also suggest `grok login` for SuperGrok)
- Session ID
- Working directory
- Model (with a model hash for coding models)
- API backend and sandbox profile (when set)
- Context window usage (used and total tokens, with the percentage used)

On the Session info tab, click a value to copy it, or drag to select a range (same highlight as the tool viewer). `c` copies the session ID and `y` copies the whole block. Copy uses the same clipboard route as the rest of Grok, including `grok wrap`.

---

## Headless Session Management

In headless mode, you manage sessions through command-line flags:

```bash
# New session each time (default)
grok-oss -p "Hello"

# Resume an existing session by ID or title (errors if it does not exist)
grok-oss -p "Continue where we left off" -r <session-id-or-title>

# Continue the most recent session in the current directory
grok-oss -p "What were we doing?" -c
```

In headless mode, resume an existing session with `-r`/`--resume`, which errors if the session does not exist, or continue the most recent session in the current directory with `-c`/`--continue`. A non-ID value is matched against session titles for the current directory, ignoring letter case (a sole manually renamed match wins among duplicates; remaining duplicates error with their IDs; UUID-shaped values always take the ID path) — scripts should pass the session ID from JSON output (see below) to `-r`.

Use `-s`/`--session-id` only to **create** a new session with a **UUID** (errors if the value is not a UUID, or if that ID already has a session under the target session directory). It does **not** resume an existing session — that was the old hidden upsert behavior; use `-r`/`-c` instead. Combine `-s` with `-r`/`-c` only when also passing `--fork-session` (forks history into a new ID; optional `-s` names the child UUID). This matches Claude Code’s anti-overwrite model (client preflight under the write cwd; sequential use is reliable, concurrent same-ID is best-effort).

To read the session ID back, request JSON output:

```bash
grok-oss -p "Hello" --output-format json | jq -r '.sessionId'
```

---

## Agent stdio Session Management

When building with ACP, sessions are managed via protocol methods:

```typescript
// Create new session
const { sessionId } = await connection.request("session/new", {
  cwd: "/path/to/project",
  mcpServers: [],
});

// Load existing session
await connection.request("session/load", {
  sessionId: "existing-session-id",
  cwd: "/path/to/project",
  mcpServers: [],
});

// Change a live option (model or reasoning_effort).
// session/new and session/load already return the typed configOptions list.
await connection.request("session/set_config_option", {
  sessionId,
  configId: "model",
  value: { value: "grok-4.6" },
});
```

The agent persists all session updates automatically. Clients can reconnect and load previous sessions by ID. See [Agent mode](15-agent-mode.md#session-config-options) for the option IDs, value shape, and leader-mode snoop.

---

## Running grok-oss sessions

**Running grok-oss sessions** lists live grok-oss TUI windows on this machine. It is not disk session history, not the `/resume` picker, not `/start`, not `/tasks`, and not the [Agent Dashboard](23-dashboard.md). Do not merge it into `/dashboard`.

```
/running
```

Alias: `/windows`. The report is a transcript table. It refreshes when you open it. It does not keep appending on a timer.

The source is `$GROK_HOME/active_sessions.json` (when `GROK_HOME` is unset, `~/.grok/active_sessions.json`). Two grok homes do not see each other. Two windows on the same conversation both appear. The row for this TUI is marked `(this window)`.

Activity is `working`, `idle`, or `unknown`. A live window with no heartbeat (an older binary) is `unknown`. That is honest, not fake idle. A title, when present, comes from the on-disk session summary (`summary.json`), never from the latest user prompt. The registry never stores prompts, tool arguments, tokens, JWTs, file contents, or message text.

Default headless processes stay unlisted unless `GROK_TRACK_HEADLESS` is already set. Leader daemons stay on `grok-oss leader list`.

From a shell:

```bash
# Human table (same columns as /running; no this-window marker)
grok-oss running

# Same filtered rows, safe fields only
grok-oss running --json
```

`grok-oss running` is not `grok-oss sessions`. The sessions subcommand is disk history (list and search). `/rebuild` still signals each live grok-oss PID once (dedupe by PID) after two windows can share one conversation.

## The grok-oss sessions Subcommand

List or search sessions from the command line. `grok-oss sessions` requires a subcommand:

```bash
# List recent sessions for the current directory
grok-oss sessions list

# Limit the number of results (default 20)
grok-oss sessions list --limit 50

# Search sessions by keyword (matches titles and prompts)
grok-oss sessions search "rate limit"
```

`grok-oss sessions list` shows sessions for the current working directory, grouped by worktree label. Each row lists the session ID, the creation and update dates, the source status, and the summary. `grok-oss sessions search` combines a local SQLite index with remote results.

---

## The grok usage Subcommand

Print persisted token and cost usage for a session. Use this instead of reading session files:

```bash
# Session totals plus every recorded turn
grok usage <session-id>

# One turn
grok usage <session-id> 3
```

Output is JSON with `sessionId`, `updatedAt`, `session`, and `turns`. A specific turn uses the same envelope with one element in `turns`. Session totals cover the whole conversation, including history inherited by resume or fork. `costUsdTicks` is 10¹⁰ ticks per USD (divide by `1e10` for dollars). A missing turn number is an error. Interactive credit and billing stay on `/usage` in the TUI.

---

## Worktree Sessions

When working with subagents or session forks, Grok can create isolated git worktrees per session. Each worktree gets its own copy of the working directory, so file changes in one session do not affect another.

Worktree sessions are managed internally through the `x.ai/git/worktree/*` extension methods. Key operations:

- **Create**: Create a new worktree for an isolated session
- **Apply**: Merge worktree changes back into the main working directory
- **Remove**: Clean up a worktree when the session is done

Resume a session in a fresh worktree with `grok-oss -w -r <session-id>`.

### Checking Disk Usage

`grok-oss du` (alias: `grok-oss disk-usage`) reports what the grok home (`~/.grok`) uses on disk. It lists each top-level directory, largest first, then each worktree with its size, type, age, label, and path. Worktrees the registry does not track appear as `untracked`. Pass `--json` for the same report as machine-readable output.

```text
Disk usage for ~/.grok
    412.3 GB  worktrees
      1.2 GB  sessions
    412.0 MB  (top-level files)
    413.9 GB  total
  Worktree clones share storage with their source, so the total can exceed real disk use.

Worktrees
        SIZE  TYPE                AGE        LABEL  PATH
    380.0 GB  session             12d ago    my-fix ~/.grok/worktrees/xai/worktree-abc
     32.3 GB  untracked (session) 40d ago           ~/.grok/worktrees/xai/worktree-old

To reclaim space, run `grok-oss worktree gc --max-age 7d --dry-run`, then the same command without `--dry-run`. Without `--max-age`, gc expires nothing.
Untracked rows are not in the registry, so gc never visits them. Remove one with `grok-oss worktree rm --dry-run <path>`, then without `--dry-run`.
```

`AGE` is the value `grok-oss worktree gc` measures: time since the worktree was last accessed, or since it was created when that is more recent. Session and agent activity update it; a shell or editor left open in the directory does not. An untracked worktree has no registry entry, so its age comes from the newest file underneath it.

Sizes are physical block counts on Unix and logical file sizes elsewhere, matching what `grok-oss worktree show` reports. A worktree clone shares storage with its source and each copy counts in full, so the total can exceed both `du -sh` and the space actually in use. When the total exceeds the used space on the volume, the report says so. `--json` carries the same figures as `volume_capacity_bytes` and `volume_available_bytes`.

The report measures a single filesystem, the one holding the grok home. A directory on any other filesystem stays out of the total and is counted in `other_filesystem_dirs`, and its worktree rows show `-` for size (`null` in `--json`). A top-level symlink to a directory, such as a relocated `worktrees`, is counted in `unfollowed_dir_symlinks`; its target stays out of the total, though the rows below it are still sized. Directories and entries the report could not read are counted in `unreadable_dirs` and `unstatable_entries`. Run `RUST_LOG=debug grok-oss du` to name each one.

Every worktree row in `--json` also carries `created_at`, `last_accessed_at`, and `last_modified_at` in unix seconds, plus `repo_name` and `git_ref`. Registry fields are `null` for untracked rows. `git_ref` is the branch recorded when the worktree was registered, not the branch checked out now.

When the registry is unavailable, every row appears as `untracked` and the report names the reason. The `--json` `registry` field carries the same value: `read`, `absent`, `busy`, `unopenable`, or `corrupt`. A `busy` registry is held by another process, so retry. An `unopenable` one has a permission or I/O problem, so check the file. A `corrupt` one is the only case that calls for deletion: remove the file the report names, then run `grok-oss worktree db rebuild`.

To reclaim space, run `grok-oss worktree gc --max-age 7d`, which removes tracked worktrees older than the age you give. Without `--max-age`, gc expires nothing, and it visits only worktrees the registry tracks. Remove an untracked worktree with `grok-oss worktree rm <path>`. Both commands take `--dry-run` and report what they would do: gc counts the worktrees it would remove, and `rm` names the path. Neither inspects the working tree for uncommitted or unpushed work, so read the preview first.

`grok worktree create [NAME]` creates the same worktree `grok -w [NAME]` would, without starting a session. It prints the directory the session would have opened in. That path is the only output on stdout, so `cd "$(grok worktree create my-fix)"` works. By default the worktree starts from HEAD with your uncommitted changes copied over. Pass `--ref <ref>` to start from a clean checkout of a branch, tag, or commit. If the new worktree lacks the directory you ran from, such as a `--ref` checkout that predates it or an ignored directory, the command prints the worktree root.

### Manage Grove redirections

A Grove worktree can redirect ignored artifact directories such as `target` and `node_modules` to storage outside the projected tree. The redirect commands take the mount path as their first argument.

```bash
grok worktree redirect list /path/to/worktree
grok worktree redirect list /path/to/worktree --json
grok worktree redirect add /path/to/worktree target bind
grok worktree redirect del /path/to/worktree target
grok worktree redirect fixup /path/to/worktree
grok worktree redirect unmount /path/to/worktree target
```

`list` prints `repo_path`, `type`, `mechanism`, `target`, `source`, and `state`. Run `unmount` without a repo-relative path to take down every redirect on the mount. Use `fixup --force` to replace Grove-owned residue. Use `fixup --strict` to refuse a populated plain directory.


```bash
grok clone https://example.com/org/repo.git --redirect-ignored
grok clone https://example.com/org/repo.git \
  --redirect-ignored --redirect-dir build --redirect-dir '**/node_modules'
grok clone https://example.com/org/repo.git --no-redirects
```

`GROVE_REDIRECTS=0` remains a runtime kill switch. Grok does not save the kill switch as the clone's redirect choice.

### Checking Disk Usage

`grok du` (alias: `grok disk-usage`) reports what the grok home (`~/.grok`) uses on disk. It lists each top-level directory, largest first, then each worktree with its size, type, age, label, and path. Worktrees the registry does not track appear as `untracked`. Pass `--json` for the same report as machine-readable output.

```text
Disk usage for ~/.grok
    412.3 GB  worktrees
      1.2 GB  sessions
    412.0 MB  (top-level files)
    413.9 GB  total
  Worktree clones share storage with their source, so the total can exceed real disk use.

Worktrees
        SIZE  TYPE                AGE        LABEL  PATH
    380.0 GB  session             12d ago    my-fix ~/.grok/worktrees/xai/worktree-abc
     32.3 GB  untracked (session) 40d ago           ~/.grok/worktrees/xai/worktree-old

To reclaim space, run `grok worktree gc --max-age 7d --dry-run`, then the same command without `--dry-run`. Without `--max-age`, gc expires nothing, and it keeps a worktree whose work it cannot find elsewhere, naming each one.
Untracked rows are not in the registry, so gc never visits them. Remove one with `grok worktree rm --dry-run <path>`, then without `--dry-run`.
```

After the grok-home table, `grok du` may print **Redirections**, **Orphaned redirections**, and **Unattributed redirect directories**. Those bytes live in Grove escape jails, not in the grok-home total. An empty scan prints nothing. Reclaim a live jail with `grok worktree clean-artifacts`. Purge live jails plus proven orphans with `grok du --clean --yes`. Delete only proven orphans with `grok du --clean-orphaned --yes`.

`AGE` is the value `grok worktree gc` measures: time since the worktree was last accessed, or since it was created when that is more recent. Session and agent activity update it; a shell or editor left open in the directory does not. An untracked worktree has no registry entry, so its age comes from the newest file underneath it.

Sizes are physical block counts on Unix and logical file sizes elsewhere, matching what `grok worktree show` reports. A worktree clone shares storage with its source and each copy counts in full, so the total can exceed both `du -sh` and the space actually in use. When the total exceeds the used space on the volume, the report says so. `--json` carries the same figures as `volume_capacity_bytes` and `volume_available_bytes`.

The report measures a single filesystem, the one holding the grok home. A directory on any other filesystem stays out of the total and is counted in `other_filesystem_dirs`, and its worktree rows show `-` for size (`null` in `--json`). A top-level symlink to a directory, such as a relocated `worktrees`, is counted in `unfollowed_dir_symlinks`; its target stays out of the total, though the rows below it are still sized. Directories and entries the report could not read are counted in `unreadable_dirs` and `unstatable_entries`. Run `RUST_LOG=debug grok du` to name each one.

Every worktree row in `--json` also carries `created_at`, `last_accessed_at`, and `last_modified_at` in unix seconds, plus `repo_name` and `git_ref`. Registry fields are `null` for untracked rows. `git_ref` is the branch recorded when the worktree was registered, not the branch checked out now.

When the registry is unavailable, every row appears as `untracked` and the report names the reason. The `--json` `registry` field carries the same value: `read`, `absent`, `busy`, `unopenable`, or `corrupt`. A `busy` registry is held by another process, so retry. An `unopenable` one has a permission or I/O problem, so check the file. A `corrupt` one is the only case that calls for deletion: remove the file the report names, then run `grok worktree db rebuild`.

To reclaim space, run `grok worktree gc --max-age 7d`, which removes tracked worktrees older than the age you give. Without `--max-age`, gc expires nothing, and it visits only worktrees the registry tracks. Remove an untracked worktree with `grok worktree rm <path>`. Both commands take `--dry-run` and report what they would do: gc counts the worktrees it would remove, and `rm` names the path.

Each run judges as many worktrees as it can in about a minute, because the same pass runs on a timer beside your session and reading a whole working tree is not free. Anything it did not reach is counted as `Not judged this pass` and waits for the next run, so on a machine with a lot to reclaim, run gc again until that number is zero.

Before removing an expired worktree, gc checks whether the removal would destroy work: uncommitted, untracked or ignored files, a commit no surviving ref holds, or state kept only in that worktree's git directory. A worktree it cannot check is kept as well. The report counts kept worktrees and names the reason, separately from the ones a live process held back. `--force` does not skip the check, and `grok worktree rm` does not apply it: it removes the path you name.

Ignored files count as work, with one exception: a directory the repository's own ignore rules exclude and that either carries a tool's cache tag or is named like one of its output directories (`target`, `node_modules`, `.venv`, and the rest). A name alone is never enough, so a hand-written `build/` nobody excluded still keeps the worktree.

A commit that only a worktree's own reflog names, which is what a `reset --hard` or an amend leaves behind, gets a lasting name under `refs/grok/reclaimed/<worktree>/<commit>` in the repository the worktree came from. Git counts a reflog as reachability when it prunes, so without that name removing the worktree is what would make the commit unreachable. Recover one with `git log refs/grok/reclaimed/` and `git branch <name> <commit>`.

Those names do not accumulate. Each gc pass drops the ones that no longer hold anything: the commit is reachable from a real ref now, or it is more than 30 days old. The report counts them as `names_collected`.

---

## Session Storage Details

### Persistence Format

Grok stores the conversation as newline-delimited JSON (JSONL). Each line in `updates.jsonl` is a self-contained ACP session update event. This format supports:

- Incremental writes (append-only during a session)
- Efficient streaming reads (for session restore)
- Easy debugging (each line is valid JSON)

The smaller state files -- `summary.json`, `plan.json`, and `signals.json` -- are plain JSON rather than JSONL. JSONL is the source of truth for session content; `grok-oss sessions search` additionally maintains a local SQLite FTS5 index over session titles and prompts for fast keyword search.

### Session Metadata

`summary.json` records, among other fields:

- `info` -- the session ID and working directory
- `session_summary` and `generated_title` -- the session summary and its model-generated title
- `title_is_manual` -- true when the title was set by a manual `/rename` (so automatic generation leaves it alone)
- `created_at` and `updated_at` -- creation and last-update timestamps
- `num_messages` and `num_chat_messages` -- update and chat-message counts
- `current_model_id` -- the model in use
- `parent_session_id` -- the source session for a fork or restore
- `agent_name` -- named agents persist this only; an inline `--agent-profile` session also persists `agent_profile` JSON
- `last_turn_summary` -- an ultra-short summary of the most recent turn
- `last_recap` -- a bounded preview of the latest session recap

### Disk Usage

Session history (`updates.jsonl`, `chat_history.jsonl`) dominates disk usage in long sessions. Use `/compact` to reduce history size.

---

## Tips

- Use `/new` to start fresh when your current context is no longer relevant.
- Use Clear finished (`[−]`, focused `X`, or `/clear-completed-todos`) to archive completed and cancelled board rows without wiping open work.
- Use `/compact` proactively in long sessions to keep the context window effective.
- Use `/rewind` to undo mistakes; it rewinds the conversation to an earlier turn (file changes from removed turns are left as-is).
- In headless mode, capture the `sessionId` from JSON output and pass it to `-r` to build multi-step automations that maintain context.
- Check `/session-info` to see how much of your context window has been used.
