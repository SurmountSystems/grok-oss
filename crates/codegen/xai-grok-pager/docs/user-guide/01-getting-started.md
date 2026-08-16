# Getting Started

**Grok OSS** (`grok-oss`) is a terminal-based AI coding assistant. It is Surmount's unofficial open-source fork of SpaceXAI Grok Build. It is not affiliated with or endorsed by xAI. Config and sessions still live under `~/.grok`.

It runs as a TUI (Terminal User Interface) that understands your codebase, executes shell commands, edits files, searches the web, and manages tasks.

You can use it interactively as a full-screen TUI, run it headlessly for scripting and CI/CD, or integrate it into editors via the Agent Client Protocol (ACP).

---

## Installation

### Grok OSS (this product)

From a clone of [SurmountSystems/grok-oss](https://github.com/SurmountSystems/grok-oss):

```bash
just install
# or: cargo install --path crates/codegen/xai-grok-pager-bin --locked --force
grok-oss --version
```

Arch users: see `packaging/aur/` for AUR PKGBUILDs (`grok-oss-git`). Nix: `nix build .#grok-oss`.

Keep a local checkout current:

```bash
# In the TUI: rebuild, signal other live product windows, re-exec this session
/rebuild

# Same install from a shell (no self re-exec)
grok-oss rebuild

# Freshness only (compare this build to Surmount main; no install)
grok-oss update --check
```

Grok OSS does not auto-download SpaceXAI binaries. Prefer `/rebuild` or `just install`. `curl https://x.ai/cli/install.sh` installs upstream **`grok`**, not this fork.

### Official upstream (SpaceXAI)

Installs the official **`grok`** binary, not Grok OSS:

```bash
curl -fsSL https://x.ai/cli/install.sh | bash
```

On **Windows (PowerShell)**:

```powershell
irm https://x.ai/cli/install.ps1 | iex
```

---

## First Launch

Start Grok OSS by running:

```bash
grok-oss
```

Bare `grok-oss` (no flags) opens the **last session for this working directory** when one exists. That is not the Welcome picker and not continue interrupted turn. First-ever use, or no last session here, stays on Welcome. Headless (`-p`) still starts fresh unless you pass `-c` / `--continue` or `--resume`. See [Session Management](17-sessions.md).

On first launch, Grok OSS opens your browser to authenticate with grok.com. After you sign in, it stores your credentials in `~/.grok/auth.json`, where they persist across sessions. Credentials refresh automatically. The product prompts you to sign in again when they can no longer be renewed.

If you prefer API key authentication (e.g., for CI/CD or environments without a browser), set the `XAI_API_KEY` environment variable instead:

```bash
export XAI_API_KEY="xai-..."
grok-oss
```

See [Authentication](02-authentication.md) for the full set of auth options including OIDC, external auth providers, and device code flow.

---

## Basic Interaction

Once authenticated, Grok presents a full-screen TUI with two main areas:

- **Scrollback** -- the conversation history showing your prompts, Grok's responses, tool calls, file edits, and more.
- **Prompt** -- the input area at the bottom where you type messages.

Type a message and press `Enter` to send it. Grok reads files, runs commands, and edits code as needed. Each tool run streams into the scrollback in real time.

Press `Tab` to move focus between the prompt and the scrollback. While a turn is running, `Esc` cancels it (the exception is fullscreen vim scrollback mode, where mid-turn `Esc` is a no-op; minimal mode cancels even with vim on); `Ctrl+C` cancels once the composer is empty — with a draft, the first press only clears it. Idle, press `Esc` twice within 800ms to clear a non-empty prompt, or (with an empty prompt and conversation messages) to open rewind — see [Keyboard Shortcuts](03-keyboard-shortcuts.md#escape). With the scrollback focused, use the arrow keys to select entries and to collapse or expand them. To navigate with `j`/`k` and fold with `h`/`l` instead, enable Vim mode.

### File References

Use `@` in your prompt to attach files:

```
@src/main.rs              # Attach a file
@src/main.rs:10-50        # Attach lines 10-50
@src/                     # Browse a directory
```

The `@` operator opens a fuzzy file picker. By default it respects `.gitignore` and hides dotfiles. Prefix with `!` to search hidden files:

```
@!.github                 # Search hidden files
@!.env                    # Attach a .env file
```

### Permissions

By default, Grok asks for permission before executing shell commands or editing files. You can approve individually or toggle always-approve mode:

- Press `Ctrl+O` to toggle always-approve mode
- Use the `--yolo` flag at launch: `grok-oss --yolo`
- Type `/always-approve` in the prompt to toggle the mode

---

## Key Concepts

### Sessions

Every conversation is a **session**. Sessions are automatically saved to `~/.grok/sessions/` and can be resumed later. Each session tracks the full conversation history, tool calls, file edits, and task state.

- Start a new session: `Ctrl+N` or `/new`
- Resume a previous session: `/resume` in the TUI, or `--resume <ID>` from the CLI
- Continue the most recent session from the CLI: `grok-oss -c` (headless or when you want `-c` explicitly). Interactive `grok-oss` already opens last session for this cwd.

### Scrollback

The scrollback is the main display area. It shows:

- **User prompts** -- your messages, rendered as sticky headers
- **Agent messages** -- Grok's responses with full markdown rendering and syntax highlighting
- **Thinking blocks** -- Grok's reasoning process (collapsible)
- **Tool calls** -- file edits (with inline diffs), command executions, search results, and more
- **Task lists** -- TODO items tracking progress

Collapse or expand the selected entry with the `Left`/`Right` arrow keys (or `h`/`l` and `e` in Vim mode). In Vim mode, press `y` to copy its content and `Y` to copy its metadata (for example, the command that ran). Press `Enter` to open it in the fullscreen viewer (in any mode).

### Tools

Grok has built-in tools for:

| Tool | Description |
|------|-------------|
| `read_file` / `search_replace` | Read and edit files with line-precise changes |
| `grep` | Regex search across your codebase (powered by ripgrep) |
| `list_dir` | List directory contents |
| `run_terminal_command` | Execute shell commands |
| `web_search` / `web_fetch` | Search the web and fetch URLs |
| `todo_write` | Create and manage task lists |
| `spawn_subagent` | Spawn parallel subagent sessions |
| `memory_search` | Search cross-session memory |

Tools can be extended with [MCP servers](05-configuration.md#mcp-servers) for integrations like GitHub, databases, and more.

### Slash Commands

Type `/` in the prompt to access commands. These provide quick actions without writing a full prompt:

```
/model grok-build                 # Switch model
/compact                          # Compress conversation history
/always-approve                   # Toggle always-approve mode
/new                              # Start a new session
```

See [Slash Commands](04-slash-commands.md) for the complete reference.

---

## Common Launch Options

```bash
# Launch the interactive TUI and submit an initial prompt as the first turn
grok-oss "fix the failing auth test and run it"

# Initial prompt in a new git worktree. Use --worktree=<name> (with `=`) so the
# prompt isn't swallowed as the worktree name. `grok-oss -w "refactor module X"`
# would treat "refactor module X" as the worktree label, not the prompt.
grok-oss --worktree=feat "refactor module X"

# Base the worktree on a specific branch (e.g. main) instead of the current HEAD:
grok-oss -w --ref main "implement feature from main"


# Start in a specific project directory
grok-oss --cwd ~/projects/my-app

# Add project-specific rules
grok-oss --rules "Always use TypeScript. Prefer functional components."

# Auto-approve all tool executions
grok-oss --yolo

# Use a specific model
grok-oss -m grok-build

# Resume a previous session
grok-oss --resume <session-id>

# Continue the most recent session (CLI). Bare interactive grok-oss already
# opens last session for this cwd.
grok-oss -c

# Experimental scrollback-native render mode. Sticky: plain `grok-oss` reopens
# in the mode last chosen via --minimal/--fullscreen (or /minimal /fullscreen).
grok-oss --minimal

# Back to the standard fullscreen TUI (and make it sticky again)
grok-oss --fullscreen

# Headless mode (for scripts)
grok-oss -p "Explain this codebase"
```

---

## Headless Mode

Run Grok non-interactively for scripting, CI/CD, and automation:

```bash
grok-oss -p "Your prompt here"
```

Output formats:

| Format | Flag | Description |
|--------|------|-------------|
| `plain` | (default) | Human-readable text |
| `json` | `--output-format json` | Single JSON object with `text`, `stopReason`, `sessionId`, and `requestId` |
| `streaming-json` | `--output-format streaming-json` | NDJSON event stream for real-time processing |

Example CI/CD usage:

```bash
grok-oss -p "Review changes for bugs" --output-format json --yolo | jq -r '.text'
```

---

## Project Rules (AGENTS.md)

Add per-project instructions by creating an `AGENTS.md` file in your repository. Grok reads these files and injects their contents as a project-instructions message at the start of the conversation:

```
~/.grok/AGENTS.md           # Global rules (apply to all projects)
<repo-root>/AGENTS.md       # Repository-level rules
<cwd>/AGENTS.md             # Directory-level rules (highest priority)
```

Deeper files take precedence. Grok also reads `CLAUDE.md` files for compatibility.

---

## Where to Go Next

| Document | What You Will Learn |
|----------|-------------------|
| [Authentication](02-authentication.md) | Browser login, API keys, OIDC, external auth, device code flow |
| [Keyboard Shortcuts](03-keyboard-shortcuts.md) | Complete reference for all key bindings |
| [Slash Commands](04-slash-commands.md) | All available `/` commands |
| [Configuration](05-configuration.md) | config.toml, pager.toml, environment variables |
