# Subagents and Personas

Subagents are independent child sessions that handle tasks in parallel. Each subagent has its own context window, so the main agent can delegate work (research, implementation, testing, and code review) without consuming its own context. A subagent reports a summary back to the parent when it finishes.

Subagents are enabled by default.

---

## Agents vs Personas

Agents and personas both customize behavior, but they operate at different levels:

| | **Agents** | **Personas** |
|---|---|---|
| **What they configure** | The whole session: model, tools, prompt mode, system prompt | A behavioral overlay added to a subagent's prompt |
| **Scope** | Primary session or subagent | Subagents only |
| **How you set them** | At startup, or with agent definitions (`.md` files in `.grok/agents/` or `~/.grok/agents/`) | In `config.toml` (`[subagents.personas]`) or `.toml` files under `.grok/personas/`; applied during subagent resolution |
| **What they control** | Model, tool availability, prompt body, skills | Tone, output format, task focus, and input/output contracts |
| **Who edits them** | You -- create, delete, or toggle them in the agents modal or by editing files | You -- define custom personas in config or files; bundled personas are read-only |
| **Examples** | `grok-build`, `explore`, `plan` | `researcher`, `concise` |

An agent defines the session itself. A persona shapes how a subagent behaves within a session. A subagent always runs as an agent type (for example, `general-purpose`), and resolution can layer a persona on top.

Manage both in the agents modal. Open it with `/config-agents` (alias `/agents`), or open the Personas tab directly with `/personas`. The modal has two tabs: **Agents** and **Personas**.

---

## Disabling Subagents

Disable subagents with a CLI flag, an environment variable, or the config file (highest priority first). The same rules apply to the interactive `grok` TUI, `grok-oss agent stdio`, and headless runs.

```bash
grok-oss --no-subagents                  # This session only
export GROK_SUBAGENTS=0              # Environment variable
```

```toml
# ~/.grok/config.toml
[subagents]
enabled = false
```

Only an explicit `enabled = false` turns subagents off. A `[subagents]` table that sets `max_depth`, `[subagents.models]`, or `[subagents.toggle]` without an `enabled` key keeps them on.

---

## How Subagents Work

When the main agent identifies work to delegate, it calls the `spawn_subagent` tool to start a child session. The child runs with:

- Its own context window, independent of the parent
- A toolset determined by its agent type and optional capability mode
- Optional persona instructions applied during resolution

The parent receives the child's output -- usually a summary -- when the child finishes.

---

## Built-in Agent Types

Built-in types still exist as host types. The model cannot pick them by name. An omitted `subagent_type` is `general-purpose`.

| Type              | Description                                          |
| ----------------- | ---------------------------------------------------- |
| `general-purpose` | Default type. Full-capability agent for any task.    |
| `explore`         | Research agent. Searches, reads, greps, and runs shell commands, but does not edit files. Use it for codebase investigation. |
| `plan`            | Planning agent. Explores the codebase and produces a structured implementation plan; does not edit files. |

Project- or user-defined agents can add new types or shadow these built-ins by name.

`spawn_subagent` has an optional `subagent_type` enum when plugin, project, or user agents exist. Its values are those agents, for example `my-plugin:reviewer`. Built-in names and xAI-bundled agents are not listed. If a parent's `tools` list restricts spawning with `Agent(...)`, the enum shows only the allowed types. An unknown type fails the call with the list of valid types.

---

## Personas

A persona is a named behavioral overlay. Its instructions are injected into the subagent's conversation as a `<system-reminder>`, which shapes tone, output format, and task focus without changing the subagent's agent type, model, or tools.

Define personas in `config.toml` or in `.toml` files:

```toml
[subagents.personas.researcher]
instructions = "You are a thorough researcher. Always cite specific file paths."
description = "Deep investigator."
```

Grok Build discovers file-based personas from these locations, in priority order:

- `.grok/personas/*.toml` (project)
- `~/.grok/personas/*.toml` (user)
- The bundled personas directory (lowest priority)

Each file defines one persona, and the file name (without the extension) becomes the persona name. Inline `config.toml` personas take precedence over files. Only `.toml` files are discovered.

Manage personas in the Personas tab of the agents modal (`/personas`). Bundled personas are read-only; personas you define are editable.

> **Note:** Grok Build applies personas through subagent resolution and roles, not through a `spawn_subagent` parameter. The main agent does not pass a persona name when it spawns a child.

### Persona Fields

| Field               | Description                                                          |
| ------------------- | ------------------------------------------------------------------- |
| `instructions`      | Inline instruction text applied as the persona layer.               |
| `instructions_file` | Path to an instruction file, loaded at spawn time and merged after `instructions`. |
| `description`       | Short summary shown in the persona catalog. Falls back to the first paragraph of `instructions`. |
| `inputs` / `outputs`| Declared input and output contract (see below).                     |
| `model`             | Model override applied when the persona is used.                    |
| `reasoning_effort`  | Reasoning effort applied when the persona is used.                  |
| `default_isolation` | Default isolation mode (`none` or `worktree`).                      |

### Input/Output Contracts

A persona can declare the inputs it expects and the outputs it produces. The parent agent reads these to know what context to supply and what artifacts to expect. This lets you chain personas, so one persona's output file becomes the next persona's input:

```toml
[[subagents.personas.reviewer.inputs]]
name = "review_file"
io_type = "file"
required = true
description = "Path to the code under review"

[[subagents.personas.reviewer.outputs]]
name = "summary_file"
io_type = "file"
required = false
description = "Path to write review notes"
```

Each field has a `name`, an `io_type` (defaults to `file`), a `required` flag, and a `description`.

### Persona Resolution

When a persona applies, Grok Build resolves the effective model and reasoning effort in this order, highest priority first:

1. Explicit spawn-time override
2. Role default
3. Persona default
4. Parent session

Isolation follows the same order for the first three steps but defaults to `none` (no worktree) rather than inheriting from the parent session.

If a persona is requested but cannot be resolved -- it is not found, has no instructions, or its `instructions_file` is unreadable -- the spawn fails.

---

## Spawning Subagents

The main agent calls the `spawn_subagent` tool. Its parameters:

| Parameter           | Description                                                       |
| ------------------- | ---------------------------------------------------------------- |
| `prompt`            | The full task prompt for the subagent.                           |
| `description`       | A short label for the task (3-5 words).                          |
| `run_in_background` | Run in the background and return a subagent ID. Defaults to `true`. |
| `isolation`         | `none` (shared workspace, the default) or `worktree` (isolated git worktree). |
| `resume_from`       | Continue a completed subagent's conversation. Pass its subagent ID. |
| `cwd`               | Working directory for the subagent. Mutually exclusive with `isolation: worktree`; ignored when `resume_from` is set (the resumed child inherits its source's directory). |

When you run a subagent in the background, retrieve its result later with `get_command_or_subagent_output`.

### Sending messages to subagents

The `send_subagent_message` tool is off by default. Enable it with `GROK_ACTIVE_AGENT_MESSAGES` or `[features] active_agent_messages`.

The root session can send a follow-up to a subagent it owns. When the flag is on, a granted child also receives the tool:

- `subagent_id: "parent"` targets that child's active parent subagent.
- A durable agent id targets another local subagent. An eligible completed subagent resumes with the same identity.

A child whose parent is the root session cannot message the root. Curated harness toolsets never receive the tool. A capability mode that excludes this kind also removes it.

Child senders are bounded: 4 in-flight messages per sender-target pair, and 32 outbound messages per sender attempt. A send over the limit returns `QuotaExceeded`.

An inactive subagent always wakes with the message as its next turn. For an active subagent, the optional `delivery` parameter controls how the message lands:

- `steer` (the default) joins the current turn at its next safe point.
- `queue` waits as a protected later turn instead of entering the active turn.
- `interject` is urgent: it is delivered ahead of pending steers at the earliest safe point, and it interrupts a subagent that is blocked waiting on background work so the subagent reads the message at once. Only the wait call ends early. The background work keeps running.

If the subagent is active but between turns, `steer` and `interject` each become one protected queued turn and the subagent starts on it. The legacy `queue: true` flag is still accepted and means `delivery: "queue"`. `delivery` wins when both are present.

The transcript shows each send as a one-line `Message` row: a verb for the outcome, then the subagent's label (its persona, role, tag, or Subagent fallback) and its description in curly quotes, as its `Subagent …: “…”` scrollback row quotes it, clamped to the first line and 40 characters. The verb carries the delivery, so a steer stays unmarked:

- `Message sent to Subagent “find callers”` (steer)
- `Message queued for Subagent “find callers”` / `Message interjected to Subagent “find callers”`
- `Message sending to …` with an animated bullet while the send is in flight
- `Message rejected · Subagent “find callers”` for a refused send, `Message unconfirmed · Subagent “find callers”` for one the shell could not confirm
- `Message sent to parent` when a child messages its parent

The collapsed row never shows the message or the reason. **Right** (or `l`/`e` in vim mode) expands the row to show the requested delivery, the full message text, and the reason of a rejected or unconfirmed send; **Left** (or `h`) collapses it again. **Enter**, **Ctrl+F**, or a double-click on the row opens that subagent's view, exactly as on its `Subagent` row (Right/Left still fold it). If the subagent was never spawned in this session (a headless `grok export`, or an id from another session), the row names it `subagent …xxxxxxxx` from the last 8 characters of its id, shows the raw `Subagent ID:` when expanded, and cannot open it.

---

## Capability Modes

Capability mode is not a spawn argument. A child's tools come from its **agent type** and any **role / definition default**. `general-purpose` is unrestricted (`all`). The built-in `explore` and `plan` types read, search, and run shell commands but cannot edit files.

| Mode         | Read | Write | Execute | Description                                  |
| ------------ | ---- | ----- | ------- | -------------------------------------------- |
| `read-only`  | Yes  | No    | No      | Read, search, and inspect (also web search and LSP); no file edits or shell. |
| `read-write` | Yes  | Yes   | No      | Read, plus create, edit, delete, and move files. No shell. |
| `execute`    | Yes  | No    | Yes     | Read, plus run shell commands and background tasks. No file edits. |
| `all`        | Yes  | Yes   | Yes     | Unrestricted tool access. Default for `general-purpose`. |

---

## Context Inheritance

### resume_from

The `resume_from` parameter lets a new subagent continue where a completed subagent left off, which is useful for multi-stage workflows:

1. Spawn a research subagent to investigate a problem.
2. Spawn a second subagent with `resume_from` set to the first subagent's ID, so it picks up with the full research context.

The new subagent inherits the source's transcript, tool state, and model; its system prompt and tools are re-rendered from the current agent definition. The source must be completed (not running), belong to the current session, and use the same agent type.

### follow_up (L1 onto a running L2)

The spawn tool's `follow_up` field is L1 enqueue onto a **still-running** nested L2. Pass that L2's subagent ID. Extra prompt text stays in `prompt`. That enqueue is additive: not kill, not respawn, not a wait for exit. It is not Operator overlay typing (open the L2 framed view and type; that is `x.ai/interject` on that L2). It is not `resume_from`, which still continues a **completed** nested L2 only. `resume_from` of a running L2 still fails. The follow-up must not inject into a live L3 unless the Operator explicitly targeted that specialist. Default on. `[subagents] parent_follow_up = false` is the SpaceXAI option (spawn, wait, `resume_from` after exit). Overlay compose stays. No new `[auth]` key.

### Early exit of an L2

If an L2 exits early, L1 resumes that same L2. Early means about a couple of minutes, few or zero tool calls, no report of the work, or a stop on a repeating sentence before the work started. L1 does not treat that exit as done. L1 does not start a second L2 for the same job. This is an L1 manager rule. It is not a follow-up into an L2 that is still running, and it is not a new L2 that only copies a finished transcript.

An L1 can read other L1 sessions on this machine. It can send those sessions a soft message about a resource they are using. A live `just check-remote` is one example. Soft means help. A soft message is not a lock, and it is not a kill.

After a Grok OSS rebuild and process restart, this Grok OSS L1 is privileged to check whether other L1s resumed gracefully and to help them along.

L1, L2, and L3 can tell the harness what failed. That includes an early exit, a repeating sentence, or a missing report.

### MCP inheritance

The primary session overlays the active agent’s `mcpServers` frontmatter onto the disk/client merge by name (agent.md headers beat `config.toml`). Switching the primary agent replaces that overlay with the new seat only. Child inline `mcpServers` still become owned clients and beat inherited shared clients. Plugin agents cannot declare `mcpServers`.

Subagents inherit the parent session’s **already-connected** MCP servers by default. That includes local stdio/HTTP servers and plugin-sourced agents (for example `my-plugin:reviewer`). The child discovers and calls those tools with `search_tool` / `use_tool` the same way the parent does.

Control inheritance with agent frontmatter `mcpInheritance`:

| Value | Effect |
| ----- | ------ |
| `all` (default if omitted) | Inherit every parent-connected MCP server |
| `none` | Inherit no parent MCP servers |
| `named: [server, …]` | Inherit only the listed server names |
| `except: [server, …]` | Inherit all parent servers except the listed names |

Example:

```yaml
---
name: research-only
description: Read MCP tools but not internal connectors
tools: search_tool, use_tool, Read
mcpInheritance:
  except:
    - internal-tools
---
```

**Plugin agents** inherit parent MCP the same way. For security they still cannot:

- Declare their own `mcpServers` in agent frontmatter (ignored with a warning)
- Declare hooks in agent frontmatter
- Set `permissionMode: bypassPermissions`

Plugin-bundled MCP servers (plugin `.mcp.json`) still attach to the **parent/session** after the plugin is trusted — they are not a child-only frontmatter declaration. See [Plugins](09-plugins.md) and [MCP Servers](07-mcp-servers.md).

---

## Isolation: Worktree Mode

By default, subagents share the parent workspace (`isolation: none`). For tasks that must keep edits separate from the parent, request an isolated git worktree with `isolation: worktree`:

- The subagent works in its own copy of the working tree.
- Its changes stay isolated from the parent until you merge them.
- The subagent's result includes the worktree path.

Grok OSS manages worktrees through the `x.ai/git/worktree/*` extension methods, including an apply operation that merges changes back into the main working directory.

Prefer no worktree when parallel subagents edit disjoint paths or when you want simpler git state.

### Worktree isolation is off by default

Grok OSS defaults `[subagents] allow_worktree` to **`false`**. Empty config means spawn **forces** `isolation = none` even if the agent requested `worktree` or a role or persona defaulted to worktree. Resume of a subagent that already has a worktree still reuses that path.

To opt in to worktree isolation:

```toml
[subagents]
allow_worktree = true
```

This row is also in `/settings` → Agent (**Allow Isolated Agent worktrees**). Earlier releases defaulted `allow_worktree` to `true`. If you relied on that, set the key above explicitly. Force-none still applies whenever the value is `false` (default or explicit).

See [Configuration](05-configuration.md) for the same key.

Mid-turn interject injects into the current turn and never cancels. Cancel is Esc or **[stop]** only. Keys and the composer Enter cue (send / queue / interject) are in [Keyboard shortcuts](03-keyboard-shortcuts.md).

---

## Configuration

### Per-Type Toggles and Model Overrides

Disable specific agent types, or route them to a different model:

```toml
[subagents.toggle]
explore = true                       # default -- omit to keep enabled
plan = false                         # disable the plan subagent

[subagents.models]
explore = "grok-4.6"                 # route explore to a specific model
```

Per-type model overrides apply for any parent. Without an override, a subagent inherits the parent's model.

### Model Selection by the Agent

The `spawn_subagent` tool offers the agent a `model` argument, and its description lists the models you can pick, for when you explicitly ask for a subagent on a different model. With `[features] subagent_model_inheritance = true` (or `GROK_SUBAGENT_MODEL_INHERITANCE=1`), both are hidden whenever every model in your picker is an xAI model: subagents then always inherit the parent's model, and a spawn that still names one fails with a message asking the agent to retry without it. Catalogs with a third-party model, a model with no declared family, or a catalog still loading keep the argument. `[subagents.models]` pins, roles, and personas are unaffected. Read when a session starts; changing it requires a restart. Precedence: a `requirements.toml`/MDM pin, then the environment variable, then `config.toml`, then remote settings, then the default (off).

You can also toggle it from `/settings` → Models → **Subagent model inheritance**:

- On: Grok cannot set models for subagents
- Off: Grok may choose a different model for a subagent. Takes effect after restart.
- NOTE: This setting only applies when all models are xAI "model_family". You likely don't need to configure this setting.

The row shows the value that applies after restart. Toggling writes `[features] subagent_model_inheritance = true` or `= false` (an explicit `false` overrides a remote `true`); `d` (reset) deletes the key so `managed_config.toml`, remote settings, or the default apply again. Agents already running keep the mode they started with. When a layer your `config.toml` cannot override decides the value — a `requirements.toml`/MDM pin, the environment variable, the `GROK_CONFIG` overlay, or an active campaign — both the toggle and the reset are refused with a toast that names that layer.

### Custom Roles and Personas

Define custom roles with their own capability and model defaults:

```toml
[subagents.roles.researcher]
description = "Deep research agent"
default_capability_mode = "read-only"
model = "grok-4.6"
prompt_file = ".grok/prompts/researcher.md"
```

Define custom personas with behavioral instructions:

```toml
[subagents.personas.concise]
instructions = "Be concise. No filler words."
# instructions_file = ".grok/personas/concise.md"  # or load from a file
```

Grok Build also discovers roles from `.grok/roles/*.toml` and personas from `.grok/personas/*.toml`. Inline `config.toml` definitions take precedence over files.

---

## The Tasks Pane (TUI)

Grok Build shows running and finished work in side panes on the agent screen:

- Press `Ctrl+G` to toggle the tasks pane, which lists active and completed subagents and background commands with their status.
- Press `Ctrl+Shift+T` to toggle the separate todo pane, or click the status-row **tasks N/M** badge. Inside a nested L2 overlay, that toggle is for the nested session's board. `Ctrl+T` expands or collapses thinking on that overlay's scrollback, same as the main session.

To view the available agent types and personas, open the command palette with `Ctrl+P` and choose **Manage Agents** (`/config-agents`).

Subagents appear at the top of the tasks pane in their own collapsible "Subagents" group.

---

## Viewing Subagents in the TUI

Subagents appear in several places in the interactive TUI:

### Scrollback (parent conversation history)

When a subagent is spawned, a compact lifecycle block is added to the *parent's* scrollback:

- `Subagent running: "do the thing" (Implementer · grok-4.6) · Thinking`
- Or for background subagents: `Subagent started: "..."`

While running, the block shows a live activity suffix (e.g. "Running: cargo test", "Compacting", "Retrying (2/3)") pulled from the child's turn tracker. The bullet animates (or is colored) according to state.

Press **Enter** (or Ctrl-F) on the block to open the subagent's full transcript.

For blocking subagents the single entry updates its bullet color when the child finishes. For background ones, a follow-up `Subagent completed/failed/cancelled in 15m43s: "..."` block is appended when the wait is a minute or more. Times under 60 seconds stay in seconds (`11.7s`). Do not write 943 seconds or `943s` in status or agent reports. SuperGrok is a paid product. When the prepaid SuperGrok top-up meter is meant, say SuperGrok dollar credits.

### Tasks pane (Ctrl+G)

As noted above — grouped under "Subagents", with spinners, elapsed times, and quick access to kill or inspect. Press `h` to toggle hide-completed / show-all.

### Dock (when enabled)

The dock above the prompt lists subagents. With the dock focused, `h` toggles hide-completed / show-all (same filter as the tasks pane). Left / Right collapse and expand a section header.

### Fullscreen framed view (the child transcript)

When you open a subagent (from a scrollback block or the tasks pane), a bordered frame replaces the parent view and shows the child's full transcript:

- Title bar inside the frame: status icon (spinner / ✓ / ✗), label + bold description + model, optional "resumed"/"forked" badge, live activity · elapsed time, and [✗] close button.
- The child's own scrollback, thinking, and tool calls render inside the frame.
- The parent tasks pane, todos pane, and dock hide for the duration of the view.

This view is observational. The composer is hidden (zero rows). You cannot focus it, type a prompt, stash a draft, or send a follow-up from here. The parent session still owns prompts. To steer a running child, close the view and use `send_subagent_message` from the parent (see [Sending messages to subagents](#sending-messages-to-subagents)).

**What still works**

- Scroll, fold, copy, open links, and open the block viewer on the child's transcript.
- `Ctrl+C` cancels **this child's** turn. It does not cancel the parent.
- `Ctrl+.` / `Ctrl+X` opens the shortcuts cheatsheet for the child's keys.
- The child view paints no `[Dashboard]` button. Inside the dashboard overlay the button is the way back.
- Idle `Enter` in the **block viewer** quotes the selected line into the parent composer and closes the view.

**What does nothing (fail closed)**

Root-only chords never start on this surface. They do not open a modal on the child, and they do not leak to the parent:

- Command palette (`Ctrl+P`), model picker (`Ctrl+M`), session picker (`Ctrl+R`)
- Settings, extensions, always-approve (`Ctrl+O`), send-to-background (`Ctrl+B`)
- External prompt editor, Shift+Tab mode cycle

A denied action is a silent redraw. There is no toast.

If a prompt-queue overlay appears, it is a **read-only mirror**. You cannot edit, send-now, or remove rows. Queue RPCs always target the parent session.

**How to leave**

- `q` or `Esc` from bare scrollback, or click [✗].
- If scrollback search is open, `q` / `Esc` closes search first. A later press closes the view.
- `Ctrl+Q` always quits Grok. It is never swallowed here.

The parent's scrollback keeps showing the subagent's status after you close.

---

## Depth Limits

The default nesting limit is two. The main thread (L1) can spawn subagents (L2). An L2 can spawn specialists (L3). An L3 at that default cannot spawn further (no L4). If the current depth is already at `max_depth`, `spawn_subagent` fails with a depth-limit error. Set `[subagents] max_depth = 1` if only the main thread should spawn. The main-thread Subagents list, watching counts, and similar live chrome show only L2 coordinators. Each L2 row shows how many live L3 specialists it is using, as a count, not a list of specialist names.

## Token efficiency

Whenever implement work, multi-file diagnosis, CI, or a regression needs tools, spawn an L2 so the main thread stays a coordinator. An L2 coordinator for implement work must spawn L3 for greps, reads, and product edits. L2 does not fill 200k implementing.

**Hierarchical fast path.** The main thread may do these three things without spawning a subagent:

1. A one-command host question (for example `journalctl` or `last`).
2. A single known-path read that you or the prompt already named.
3. Read and quote the short on-disk report that this thread asked for.

Write the short report under `~/.agents/reports/` on this machine. Do not add report files to the git tree.

That is not a license to diagnose or implement in the main thread.

| Depth | Does | Does not |
| ----- | ---- | -------- |
| **L1 main** | Status to you. Spawn L2. Wait. Read short reports. Update the session board. Hierarchical fast path. | Diagnose, implement, multi-file reads, CI logs |
| **L2 subagent** | Parallelize. Spawn L3 for greps, reads, and product edits; do not fill 200k implementing. Explore and plan L2 stay read-only specialists that may still grep and read. Stay token-efficient. Discard context after a report goes up. Operator-facing nested view: you can ask or clarify in that L2 overlay. | Spawn L4. Show raw edits as if this were the main thread. |
| **L3 specialist** | All actual tools and work, in parallel. Same agency as L2 except it cannot spawn. | Spawn L4 (forbidden). Operator chat does not barge into a live L3. |

L3 is not a weaker agent. L3 still does the tools. The hard cap is no L4. L2's extra job versus L3 is spawning L3 specialists and being the nested view you talk to. L3's extra job versus L2 is doing the tools.

Spawn an L2 when the job needs isolation from the main thread: implement work, multi-file diagnosis, CI, regressions, skill-maintenance, or any tool work that would fill the parent. The Hierarchical fast path does not spawn an L2. An additive "also" or "btw" ask spawns another L2, or queues if it would write the same files. Do not kill a healthy L2 that is already running.

L2 parallelizes, spawns L3 for greps, reads, and product edits, waits, reads the L3 short reports, and writes one L2 report under `~/.agents/reports/` on this machine. L2 does not fill 200k implementing. The main thread reads that L2 report only and talks to you. It does not re-do the L3 greps. Those files are reports, not joins. They are not part of the git tree.

The main (L1) session uses the catalog 500k sampling window. AUTO compact on L1 uses that window, not the old 200k knee. Nested L2 sampling stays 200k. L2 may compact (95% of nested 200k). Nested L3 sampling also stays 200k. An L2 may compact a specialist it spawned when that specialist is near the 200k sampling window. The specialist must not compact itself and continue. An L3 is disposable. If it stalls or spirals, stop it. When an L3 is near 200k, it summarizes, reports to L2, and stops. Keep about 40% of the window that session is running: 40% of 500k on L1, 40% of 200k on nested agents. Nested agents throw context away after a report, so they do not need 500k. The footer context chip names the sampling window that session actually uses.

L1 and L2 may still spawn subagents, update the session board, wait on specialists, and read the short on-disk report they asked for. That is coordination, not work. L2 exists so its context can be thrown away after the report. An L2 coordinator for implement work must spawn L3 for greps, reads, and product edits. L2 does not fill 200k implementing. Ordinary L2 still AUTO compact at 95% of nested 200k.

See [Configuration → Token Economy](05-configuration.md#token-economy) for economic mode, implement-effort Settings, and ASCII scrub. `/spend` and `/limits` are the live views. Isolated Agents are not free. They help when the main thread stays small.

## Live job actual tokens

Actual tokens on a live job row come from the host session-usage figure for that row. That figure is not included SuperGrok period limits, not SuperGrok dollar credits, and not console team prepaid / console API credits. SuperGrok is a paid product.

If the host has not returned a figure yet, the row omits the token clause. It does not print a placeholder. The wall-clock estimate and the token estimate stay labeled as estimates. Do not copy the estimate into the token count. A standing estimate such as 19.4 minutes and 167.0k is still an estimate until the host returns a figure. Do not invent a count.

When the host has returned a figure, the row shows that figure. Showing the row does not add that figure again into the L1 total or into the grok-oss sqlite session record. Nested spend stays on the nested row. The footer sampling window for the main thread is not that figure.

The tasks pane paints each running job row by calling `display_live_job_row` and showing that row's labeled estimates and, when the host returned a figure, that figure. `/tasks` uses the same formatter. Neither path adds the row figure to the L1 total or to the grok-oss sqlite session record.

---

## When to Use Subagents

**Good use cases:**

- Researching a codebase while the main thread stays free for you
- Running tests in parallel with other specialists
- Reviewing generated changes before you commit them
- Delegating independent tasks that do not depend on each other

**When not to use:**

- Tasks that require tight back-and-forth with the user, since a subagent runs autonomously and isn't suited to interactive exchanges
- Tasks where the Isolated Agent setup cost exceeds the parallelism benefit, and the **Hierarchical fast path** already covers the job (one-command host question, a single known-path read already named, or reading the asked-for report). Implement work, multi-file diagnosis, CI, and regressions still spawn an L2. L2 spawns L3 only if the problem is actually hard.
