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

Disable subagents with an environment variable or the config file:

```bash
export GROK_SUBAGENTS=0              # Environment variable
```

```toml
# ~/.grok/config.toml
[subagents]
enabled = false
```

---

## How Subagents Work

When the main agent identifies work to delegate, it calls the `spawn_subagent` tool to start a child session. The child runs with:

- Its own context window, independent of the parent
- A toolset determined by its agent type and optional capability mode
- Optional persona instructions applied during resolution

The parent receives the child's output -- usually a summary -- when the child finishes.

---

## Built-in Agent Types

The `spawn_subagent` tool accepts a `subagent_type` parameter that selects the child's role:

| Type              | Description                                          |
| ----------------- | ---------------------------------------------------- |
| `general-purpose` | Default type. Full-capability agent for any task.    |
| `explore`         | Research agent. Searches, reads, greps, and runs shell commands, but does not edit files. Use it for codebase investigation. |
| `plan`            | Planning agent. Explores the codebase and produces a structured implementation plan; does not edit files. |

Project- or user-defined agents can add new types or shadow these built-ins by name.

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

| Parameter         | Description                                                       |
| ----------------- | ---------------------------------------------------------------- |
| `prompt`          | The full task prompt for the subagent.                           |
| `description`     | A short label for the task (3-5 words).                          |
| `subagent_type`   | The agent type to launch. Defaults to `general-purpose`.         |
| `background`       | Run the subagent in the background and return immediately with a subagent ID. Defaults to `false`. |
| `capability_mode` | Restrict the subagent's tools: `read-only`, `read-write`, `execute`, or `all`. |
| `isolation`       | `none` (shared workspace, the default) or `worktree` (isolated git worktree). |
| `resume_from`     | Continue a completed subagent's conversation. Pass its subagent ID. |
| `cwd`             | Working directory for the subagent. Mutually exclusive with `isolation: worktree`; ignored when `resume_from` is set (the resumed child inherits its source's directory). |

When you run a subagent in the background, retrieve its result later with `get_command_or_subagent_output`.

---

## Capability Modes

A capability mode is an optional, coarse filter on a subagent's tools:

| Mode         | Read | Write | Execute | Description                                  |
| ------------ | ---- | ----- | ------- | -------------------------------------------- |
| `read-only`  | Yes  | No    | No      | Read, search, and inspect (also web search and LSP); no file edits or shell. |
| `read-write` | Yes  | Yes   | No      | Read, plus create, edit, delete, and move files. No shell. |
| `execute`    | Yes  | No    | Yes     | Read, plus run shell commands and background tasks. No file edits. |
| `all`        | Yes  | Yes   | Yes     | Unrestricted tool access.                    |

If you omit `capability_mode`, the subagent uses its agent type's toolset. The built-in `explore` and `plan` types read, search, and run shell commands but cannot edit files; `general-purpose` ships the full toolset.

---

## Context Inheritance

### resume_from

The `resume_from` parameter lets a new subagent continue where a completed subagent left off, which is useful for multi-stage workflows:

1. Spawn a research subagent to investigate a problem.
2. Spawn a second subagent with `resume_from` set to the first subagent's ID, so it picks up with the full research context.

The new subagent inherits the source's transcript, tool state, and model; its system prompt and tools are re-rendered from the current agent definition. The source must be completed (not running), belong to the current session, and use the same agent type.

### MCP inheritance

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

Grok Build manages worktrees through the `x.ai/git/worktree/*` extension methods, including an apply operation that merges changes back into the main working directory.

**Prefer no worktree** when parallel children edit disjoint paths or when you want simpler git state. Skills that orchestrate subagents should default to `isolation: none` unless the user asks for a worktree.

### Worktree isolation is off by default

Surmount OSS defaults `[subagents] allow_worktree` to **`false`**. Empty config means spawn **forces** `isolation = none` even if the agent requested `worktree` or a role/persona defaulted to worktree. Resume of a child that already has a worktree still reuses that path.

To **opt in** to worktree isolation:

```toml
# ~/.grok/config.toml
[subagents]
allow_worktree = true
```

**Migration:** earlier releases defaulted `allow_worktree` to `true`. If you relied on that, set the key above explicitly. Force-none still applies whenever the value is `false` (default or explicit).

---

## Configuration

### Per-Type Toggles and Model Overrides

Disable specific agent types, or route them to a different model:

```toml
[subagents.toggle]
explore = true                       # default -- omit to keep enabled
plan = false                         # disable the plan subagent

[subagents.models]
explore = "grok-build"               # route explore to a specific model
```

Per-type model overrides apply for any parent. Without an override, a subagent inherits the parent's model.

### Custom Roles and Personas

Define custom roles with their own capability and model defaults:

```toml
[subagents.roles.researcher]
description = "Deep research agent"
default_capability_mode = "read-only"
model = "grok-build"
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
- Press `Ctrl+T` to toggle the separate todo pane.

To view the available agent types and personas, open the command palette with `Ctrl+P` and choose **Manage Agents** (`/config-agents`).

Subagents appear at the top of the tasks pane in their own collapsible "Subagents" group.

### Queue hold while subagents run

Queued follow-ups **hold** not only when the parent is blocked waiting on a subagent, but also whenever **any background subagent is still live** — even if the parent already looks idle. That keeps typed follow-ups from starting a conflicting main turn while children work.

- Status cue: e.g. `N subagent(s) still running · M queued — Interject to force`.
- **Interject** (chord or queue row `[Interject]`) force-starts the next parent turn despite live children.
- When the last holding subagent finishes, the queue drains on its own (no extra keystroke).
- **Monitors** and long-running background commands alone do **not** hold the queue (they can run indefinitely).

For **annotations that must not become a turn at all**, use [`/note`](04-slash-commands.md#note) instead of typing into the composer or queue. Session notes are operator-local and never drain as prompts.

See also [keyboard shortcuts — during an active turn](03-keyboard-shortcuts.md#during-an-active-turn-agent-running).

---

## Viewing Subagents in the TUI

Subagents appear in several places in the interactive TUI:

### Scrollback (parent conversation history)

When a subagent is spawned, a compact lifecycle block is added to the *parent's* scrollback:

- `Subagent running: "do the thing" (Implementer · grok-3) — Thinking`
- Or for background subagents: `Subagent started: "..."`

While running, the block shows a live activity suffix (e.g. "Running: cargo test", "Compacting", "Retrying (2/3)") pulled from the child's turn tracker. The bullet animates (or is colored) according to state.

Press **Enter** (or Ctrl-F) on the block to open the subagent's full transcript.

For blocking subagents the single entry updates its bullet color when the child finishes. For background ones, a follow-up `Subagent completed/failed/cancelled in Xs: "..."` block is appended.

### Tasks pane (Ctrl+G)

As noted above — grouped under "Subagents", with spinners, elapsed times, and quick access to kill or inspect.

### Fullscreen framed view (the child transcript)

When you open a subagent (from a scrollback block or the tasks pane), the parent view is replaced by a bordered frame containing the child's full transcript:

- Title bar inside the frame: status icon (spinner / ✓ / ✗), label + bold description + model, optional "resumed"/"forked" badge, live activity · elapsed time, and [✗] close button.
- The child's own scrollback, thinking, tool calls, and (limited) prompt area render inside the frame.
- Subagent views are largely observational — you generally cannot send new top-level prompts directly to them the way you can a parent session.

Use `q`, `Esc`, or click the close button to pop back to the parent view. The parent's scrollback continues to show the subagent's status.

---

## Depth Limits

Only the top-level session spawns subagents. A subagent cannot spawn its own subagents: the maximum nesting depth is one. If a subagent calls `spawn_subagent`, the call fails with a depth-limit error. This keeps the agent tree flat and prevents runaway spawning.

---

## Token efficiency

The parent session’s context is expensive. Filling it with logs, greps, and long file reads hurts quality long before the hard limit, and a parent compact can wipe coordination detail. Use subagents so heavy work runs in a fresh window; keep the parent as coordinator.

### Parent coordinates; children do the heavy work

- **Parent keeps:** goals, spawn/wait, short on-disk join notes, status to you.
- **Children own:** multi-file search, CI log fetch, root-cause reads, implementation, and review loops.
- Prefer **tight prompts** (goal, paths, acceptance, where to write a short summary). Prefer **short returns** (verdict, paths, residuals) over pasting whole transcripts into the parent.

### Parent is coordinator only (spawn first)

On a **CI failure**, **regression**, or **multi-file diagnosis**, the parent’s **first** tool use should be spawning a tightly scoped subagent — not pulling CI logs, opening failing tests, or grepping the hot path in the parent.

| Parent may | Parent should not (even “just a quick look”) |
| ---------- | --------------------------------------------- |
| Set goals, spawn/wait children | Pull CI logs or open failing test files first |
| Read short on-disk summaries children wrote | Re-run the child’s greps “to be sure” |
| One-line status to you | Solo marathons of multi-file research or fix work |

**Failure mode to avoid:** parent greps docs + fetches logs + finds the test file, *then* spawns. That already burned the parent context. Spawn first; children own fetch/read/fix.

### Join on disk

Children write short summary or review files. The parent reads those only — not full child transcripts and not whole hot modules after a summary already named the set. After compaction, reseed from on-disk artifacts rather than re-exploring from zero.

### Depth is one

Children cannot spawn children (see [Depth Limits](#depth-limits)). Hierarchical work means the **parent** layers specialists in sequence or parallel (inventory → root cause → fix → verify), each with a tight prompt and an on-disk artifact.

### Soft quality band

Treat the parent as a **coordinator budget**, not a place to hoard tool output. Soft guidance: keep parent context lighter when you can (quality often drops well before the hard cap). Child isolation is the win: each subagent starts fresh so deep loops do not force an early parent compact.

### Parallelism without waste

Spawn for **independent**, **disjoint** scopes when the join is cheap. Do not fan out many identical explores over the same files, or spawn for pure status checks with no real work. Cap concurrent children when scopes would otherwise overlap.

Skills and project agent rules (for example `AGENTS.md` in a repo, or your host agent config) may pin a stricter “hard stop” for operators — this section is the product-facing summary those rules link to.

---

## When to Use Subagents

**Good use cases:**

- Researching a codebase while the parent continues other work
- Running tests in parallel while the parent implements changes
- Reviewing generated changes before you commit them
- Delegating independent tasks that do not depend on each other
- CI failures, regressions, and multi-file root cause (spawn first; join on short summaries)

**When not to use:**

- Simple tasks that the parent can handle directly
- Tasks that require tight back-and-forth with the user, since a subagent runs autonomously and isn't suited to interactive exchanges
- Tasks where the context setup cost exceeds the parallelism benefit
