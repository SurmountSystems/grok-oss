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
  resources_state.json    # live TODO / tool Resources snapshot
  plan.json               # TODO/task list state (snapshot / fallback)
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

`prompt_wal.jsonl` sits next to `unsent_prompt_draft` in that session directory. It is not git, not conversation, and not model tokens. Compact does not rewrite it. Each line is one operator event (Enter send, mid-turn interject, queue enqueue, plan Human-box notes that ride Approve, or a `/rebuild` persist flush) with a ULID, wall time, session id, kind, full operator text, and `[Image #N]` file ids under `images/` (never inline data URLs). If a crash or rebuild drops a send that is missing from chat history, prompt history, and the queue, session load restores that WAL send as a pending Human turn.

See [`/rebuild`](04-slash-commands.md#rebuild) for how that persist path writes the WAL. Nested work on the leader survives `/rebuild` the same way a TUI disconnect does (the leader process stays up while nested ids are live).

Token Economy books live in `$GROK_HOME/grok_oss.db`, not in the session tree. Override the path with `[token_economy] grok_oss_database_path` in toml only. There is no Settings row for that path. See [Configuration](05-configuration.md#token-economy).

### Continue interrupted turn on restart

This is **not** last-session-on-start, and it is **not** the `/resume` picker.

**Last session for this directory** reopens the conversation when you launch bare `grok-oss`. **Continue interrupted turn** is what happens **inside** a reopened session when the last top-level turn was cut short mid-work.

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

## The session todo board

The live session board is the TODO list. `resources_state.json` is the live snapshot. `plan.json` is a resume fallback. Open the pane with `Ctrl+Shift+T`, or click the status-row **tasks N/M** badge. The pane starts closed. A nested overlay shows that nested session's board, not the parent L1 list. `Ctrl+T` expands or collapses thinking.

When the board is open and at least one completed or cancelled row exists, the todo header shows compact **`[−]`** (U+2212 minus) next to close. The icon paints whether or not the todo pane has keyboard focus. It does not paint when the board is hidden or nothing is finished.

Click that icon, press `X` while the todo pane is focused, or run `/clear-completed-todos`. Those paths archive finished rows off the live board (**Clear finished**). Pending and in-progress items stay. This is not `h` hide-done, and it is not a `merge: false` wipe of open work. Hints still say **Clear finished**. The chrome itself is the compact minus, not the long words.

---

## Resuming Sessions

### From the TUI

Use the `/resume` command to browse and resume previous sessions:

```
/resume
```

This opens a session picker that lists recent sessions for the current workspace. Select a session to resume it. The command takes no arguments.

Typing in the picker filters the list by title and also searches your conversation content as you type; content matches appear under an "Extended search results" heading. Press `Ctrl+/` to search immediately without the brief pause.

For the live top-level sessions in this pager (parent and forks), switch, rename, peek, dispatch, or close them with the [Agent Dashboard](23-dashboard.md): `/dashboard` (aliases `/sessions`, `/agents-dashboard`) or `Ctrl+\`.

To see other live grok-oss TUI windows on this machine, including a second window on the same conversation, use [Running grok-oss sessions](#running-grok-oss-sessions) (`/running`, alias `/windows`). That list is not the Agent Dashboard and not the `/resume` picker.

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
/compact [context]
```

The optional `context` argument lets you provide additional instructions about what to preserve during compaction.

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
- Auth method (OAuth vs API key; API-key sessions also suggest `grok-oss login` for SuperGrok)
- Session ID
- Working directory
- Model (with a model hash for coding models)
- API backend and sandbox profile (when set)
- Context window usage (used and total tokens, with the percentage used)

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
```

The agent persists all session updates automatically. Clients can reconnect and load previous sessions by ID.

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
- `created_at` and `updated_at` -- creation and last-update timestamps
- `num_messages` and `num_chat_messages` -- update and chat-message counts
- `current_model_id` -- the model in use
- `parent_session_id` -- the source session for a fork or restore
- `agent_name` -- the agent definition active when the session was last saved

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
