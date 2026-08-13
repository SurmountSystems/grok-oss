# Keyboard Shortcuts

Reference for key bindings in the Grok Build TUI. Bindings are built in and cannot currently be remapped.

---

## Input Modes

Grok has two input modes that control how you navigate the scrollback:

- **Simple mode** (default): Arrow keys for navigation, `Shift+Arrow` for turn navigation, `Space` to focus the prompt, and any letter key auto-focuses the prompt.
- **Vim mode** (opt-in): `j`/`k` for navigation, `H`/`L` for turn navigation, `J`/`K` for response navigation, `h`/`l` for fold, `e`/`E` for expand/collapse, and `i`/`Tab`/`Space` to focus the prompt.

Simple mode is active by default. To switch to Vim mode, set `vim_mode = true` under `[ui]` in `~/.grok/config.toml`, or toggle it at runtime with `/vim-mode`. See [Configuration](05-configuration.md) for details.

The tables below document bindings for both modes. The "Key" column shows the Vim-mode binding, and the "Alt Key" column shows the equivalent in simple mode (arrow keys, etc.).

> **Vim-mode required**: Single-letter and `Shift+letter` bindings in the
> **Scrollback** context (`j/k`, `h/l`, `g/G`, `L/H`, `y/Y`, `o/O`, `r`,
> `x`, `e/E`, and the `i` insert-mode alt) require `[ui].vim_mode = true`
> in `~/.grok/config.toml` (or `/vim-mode` to toggle). Arrow keys, `Tab`,
> `Esc`, `Space`, `PageUp/Down`, and every `Ctrl+letter` shortcut work in
> both modes.

---

## Navigation (Scrollback Focused)

Move through conversation entries in the scrollback pane.

| Key | Alt Key | Action |
|-----|---------|--------|
| `j` | `Down` | Select next entry |
| `k` | `Up` | Select previous entry |
| `⇧L` | `Shift+Right` | Jump to next turn (user prompt) |
| `⇧H` | `Shift+Left` | Jump to previous turn (user prompt) |
| `⇧J` | | Jump to next assistant response |
| `⇧K` | | Jump to previous assistant response |
| `g` | | Go to top of scrollback |
| `⇧G` | | Go to bottom of scrollback |
| `Ctrl+K` | | Scroll up one line (without changing selection) |
| `Ctrl+J` | | Scroll down one line (without changing selection) |
| `PageUp` | | Scroll up one page (selection moves to the top of the viewport) |
| `PageDown` | | Scroll down one page (selection moves to the bottom of the viewport) |
| `Ctrl+U` | | Scroll up half page |
| `Ctrl+D` (`Shift+D` in VSCode) | | Scroll down half page |

`PageUp` and `PageDown` also scroll the conversation while the ordinary prompt
is focused, without moving focus or changing the draft. An active prompt
history, `@` file search, slash menu, or completion dropdown keeps the keys for
its own navigation.

---

## View (Scrollback Focused)

Control how entries are displayed in the scrollback.

| Key | Alt Key | Action |
|-----|---------|--------|
| `h` | `Left` | Collapse selected entry |
| `l` | `Right` | Expand selected entry |
| `e` | | Toggle fold on selected entry |
| `⇧E` | | Expand all / collapse all entries |
| `Ctrl+E` | | Expand/collapse all thinking blocks |
| `r` | | Toggle raw markdown on selected entry |

Setting `respect_manual_folds = true` under `[scrollback.scroll]` in
`pager.toml` (opt-in, off by default — see
[Configuration](05-configuration.md)) makes a hand-folded block pinned:
streaming updates and finish events (for example a thinking block ending)
leave it alone instead of resetting it, and expanding a block while
auto-scroll is following the tail stops following so you can read; resume
with `⇧G`, `j` at the last entry, scrolling past the bottom, or sending a new
prompt. `⇧E` clears all pins, and `Ctrl+E` clears pins on thinking blocks.

### Block Content

| Key | Action |
|-----|--------|
| `y` | Copy block content to clipboard (also: selection-box **`⧉`** when a copyable block is selected; default on via `selection_buttons`. Mouse: click always-on **`⧉`** on a user or assistant message bubble to copy that bubble without selecting first; `bubble_copy_buttons`, default on) |
| `⇧Y` | Copy block metadata (e.g., the shell command) to clipboard |
| `Enter` | Open block content in fullscreen viewer |
| `Ctrl+F` | Open block content in fullscreen viewer (alt binding) |

---

## Focus

Switch between the prompt input and scrollback pane.

| Key | Alt Key | Context | Action |
|-----|---------|---------|--------|
| `Tab` | `Space` (and `i` in vim mode) | Scrollback focused | Focus the prompt input |
| `Tab` | | Prompt focused | Focus the scrollback (both simple and vim scrollback modes) |
| `Enter` | | Prompt focused | Submit the current prompt: **send** when idle and nothing holds the queue; **queue** while a turn is running or background subagents hold drain; **soft-interject** the top queued follow-up when the composer is empty mid-turn. The footer label (`Enter: send` / `queue` / `interject`) always matches this outcome. |

**Esc is not a focus key.** It follows the cancel / clear / rewind semantics below. The mid-turn cancel is the only branch gated on `[ui].vim_mode` (scrollback nav); nothing depends on `[ui].simple_mode` (prompt editor). Overlays, modals, slash/file dropdowns, voice, search, and selection still steal Esc first.

## Blocking cards

Three surfaces block the agent on your answer and take over the keyboard while
they are open: the **question card** (`ask_user_question`), the **permission
prompt**, and the **cancel-turn panel**. When more than one is open the
permission prompt has the keyboard first, then the cancel-turn panel, then the
question card — and the shortcuts bar always shows the keys of whichever one is
receiving them.

They share one contract:

- `Tab` / `Shift+Tab` walk that card's rows and wrap at both ends. They never
  move focus out of the card, so the cursor is always somewhere you can see.
- `Esc` steps back out, one rung at a time: it clears whatever the card has
  pending first, and only once there is nothing left to clear does it leave.
  Where it leaves to is the one thing that differs per card — the question card
  and the permission prompt park the keyboard in the scrollback so you can
  scroll up and read the context behind them (the card stays on screen), while
  the cancel-turn panel's "keep running" closes the panel and leaves the
  turn (and any subagents) running. Enter or `1`–`4` still pick a
  cancel-and-subagent choice.
- With the keyboard parked, the shortcuts bar shows the scrollback's own keys,
  and its focus hint names the card rather than the prompt: `Tab/Space:
  question`. That hint is pinned, so a narrow bar can never trim away the only
  route back.
- Inside the dashboard's session overlay there is one more rung: once the
  keyboard is parked, the next `Esc` returns to the dashboard, leaving the card
  pending. (`Ctrl+\` still leaves from any state.)

### Question card (`ask_user_question`)

| Key | Action |
|-----|--------|
| `↑` / `↓`, `j` / `k` | Move between answers (clamped at the ends) |
| `Tab` / `Shift+Tab` | Walk this question's answers in a loop — off the last answer back to the first. It never carries you into another question |
| `←` / `→`, `h` / `l`, `[` / `]` | Previous / next question |
| `1`–`9`, `a`–`f` | Pick that answer directly |
| `z` | Jump to the free-text row and start typing |
| `Space` | Toggle the focused answer (multi-select), or start typing on the free-text row |
| `Enter` | Select and advance, submit on the last question, or edit the free-text row |
| `Esc` | Unselect this question's answer; with nothing selected, park focus in the scrollback (`Tab` returns). On the *first* question inside the dashboard's session overlay it returns to the dashboard instead — from a later question `←` is still the way back, so the park comes first and the next `Esc` leaves. The shortcuts bar names whichever rung is live |
| `y` | Copy the focused answer |
| `Shift+X` | Dismiss the question (the agent continues without an answer) |
| `Ctrl+F` | Fullscreen the card |

The bare `/feedback` pane is the one exception to this table: it has no answers
to walk, `Enter` sends the report, and `Esc` dismisses the pane.

While typing a free-text answer, `Enter` submits and `Esc` returns to the
answer rows; every other key goes to the text field.

### Permission prompt

| Key | Action |
|-----|--------|
| `↑` / `↓`, `j` / `k` | Move between options (clamped at the ends) |
| `Tab` / `Shift+Tab` | Walk the options in a loop |
| `1`–`9` | Choose that option directly |
| `Enter` | Choose the focused option |
| `←` / `→` | Widen / narrow the scope an "always" answer would remember |
| `e` | Edit the always-allow pattern by hand (bash prompts) |
| `Ctrl+F` | Expand / collapse the full arguments |
| `Ctrl+O` | Turn on always-approve mode |
| `Esc` | Park focus in the scrollback (`Tab` returns). It never answers or dismisses the request |
| `Ctrl+C` | Cancel the request |

Typing on the "No" row starts a message back to the agent instead; `Enter`
sends it and `Esc` returns to the options.

### Cancel-turn panel

| Key | Action |
|-----|--------|
| `↑` / `↓`, `j` / `k`, `Tab` / `Shift+Tab` | Move between the choices |
| `1`–`4`, `Enter` | Confirm that choice |
| `Esc` | Keep everything running. This resolves the panel, so it is never a dead end and never needs to park |

## Escape

| State | Gesture | Effect |
|--------|---------|--------|
| Turn running, **minimal mode or vim scrollback mode off (the default)** | **2× `Esc` within 800ms** | Hard-cancel the turn (same outcome as the status-row **`[stop]`** control). Works with prompt or scrollback focused, even with a draft — the draft is **preserved**, unlike Ctrl+C's clear-first gesture. First press shows “press again to cancel” and does **not** cancel yet, so Esc that only closed a dialog or dropdown cannot also stop the turn. |
| Turn running, **fullscreen vim mode** | `Esc` | Swallowed no-op (does **not** cancel). Use `Ctrl+C` (or palette / other cancel entry points). |
| **Subagents still running** cancel panel open | `Esc` | **Dismiss only** — closes the panel and leaves the parent turn (and subagents) running. Cancel proceeds only via an explicit choice (Enter / 1–4 / click). |
| Turn cancelling | `Esc` | Re-sends cancel in **every** mode (retry if the first ack was lost; no double-Esc arm). `Ctrl+C` in this state escalates toward quit. |
| Idle + non-empty prompt (text or image chips), **prompt focused** | **2× `Esc` within 800ms** | Clear the prompt; non-empty text is saved to prompt history. First press shows “press again to clear”. |
| Idle + empty prompt + conversation messages, **prompt or scrollback focused** | **2× `Esc` within 800ms** | Open the rewind picker (same as `/rewind`). First press is silent (no toast). |
| Idle + empty + no messages, **or scrollback focused with a draft / moded (`!` `#` feedback) composer / pending needs-input overlay / open history search** | `Esc` | Swallowed no-op (does not focus scrollback). Clear is prompt-pane only; rewind requires an empty Normal-mode composer, no pending overlay, and no open history search — reading the scrollback never mutates your draft, your composer mode, a question awaiting an answer, or an in-progress search. |

**Post-cancel grace:** for about a second after an Esc-triggered cancel, the idle rewind arm stays suppressed — mashing Esc to stop a turn cannot silently open the rewind picker. Only the rewind arm is held; every other Esc behavior is unaffected.

**Steal-Esc (runs before mid-turn cancel / swallow and clear / rewind):** overlays, modals (including the **subagents cancel panel** — Esc dismisses without cancelling), slash/file/completion dropdowns, history search, scrollback search, text selection, link highlight, voice, and **Bash / Remember / Feedback mode exit** when the prompt is empty (Esc leaves `!` / `#` / feedback mode and returns to the normal prompt — even while a turn is running).

**Ctrl+C vs Esc:** with a non-empty draft while a turn is running, Ctrl+C clears the draft and keeps the turn; a second Ctrl+C on an empty prompt cancels. Esc requires two presses within 800ms to cancel and preserves the draft (in fullscreen vim mode it does not cancel — it only retries while already cancelling). Idle non-empty Ctrl+C clears in one press; idle Esc also requires two presses within 800ms (clear or rewind).

---

## Agent-Level

Actions that affect the agent session, available from the agent screen.

| Key | Context | Action |
|-----|---------|--------|
| `Ctrl+P` | Agent screen | Open the command palette |
| `?` (Shift+/) | Agent screen | Open the command palette (alt binding) |
| `Ctrl+M` | Agent screen | Open the model picker / switch model |
| `Ctrl+M` | Prompt focused | Toggle multiline input mode |
| `Ctrl+C` | Agent screen | **Hard cancel** the current turn (or clear a non-empty draft first; see Escape table). Same action as the turn-status **`[stop]`** button (red on hover) while a turn is running. When the primary session is idle but background subagents are still running, cancel opens the “Subagents are still running” panel or stops them according to your cancel preference. This is not global pause and not soft stop. |
| `Ctrl+Shift+Space` | Always (any screen) | **Pause or resume all work** across every open session in this process (not only the focused one). On a mouse host the turn-status row also paints a quiet **`[pause]`** control while a turn or subagents are live; while paused the same control becomes **`[resume]`** (quiet white on hover, never red). Global pause cancels in-flight turns, holds queues, then resumes only interrupted mid-turn prompts and already-queued work; finished sessions are not re-spawned. A toast tracks how long you have been paused. Bare `Space` still focuses the prompt / types spaces; voice dictation stays on `Ctrl+Space`. |
| `Ctrl+Shift+S` | Always (any screen) | **Soft stop:** arm so that after the **current** top-level turn finishes (success or terminal fail), further **queued** work does not start. Does **not** cancel mid-flight (unlike fearless pause or hard `[stop]`). There is **no** soft-stop button on the status row in this release; the control is chord-only. Status toast shows armed vs queue held. Press again before the turn ends to disarm, or after hold to release the queue. Does not steal `Ctrl+Shift+Space`. |
| `Ctrl+O` | Agent screen | Toggle always-approve (YOLO) mode |
| `Ctrl+S` | Agent screen | Open the session picker (resume a previous session) |
| `Ctrl+;` (alt: `Ctrl+'`) | Agent screen | Toggle the prompt queue pane (when non-empty). **Local macOS** VS Code family only: primary **`Ctrl+4`** (`;` / `'` still alts). SSH and non-Mac keep **`Ctrl+;`** / **`Ctrl+'`**. |
| `Shift+Tab` | Prompt focused | Cycle mode (Normal → Plan → Always-approve) |
| `Ctrl+B` | Agent screen | Send the running foreground command to the background |
| `Ctrl+T` | Agent screen | Toggle the todos pane |
| `h` | Todo pane focused | Hide or show completed/cancelled rows in the pane only (view filter; does not change the board or badge) |
| `X` | Todo pane focused | Optional **Clear finished** accelerator — remove completed and cancelled items and archive them. Prefer the pane **clear-finished icon** (`[−]`, when the todo board is open and finished rows exist; quiet idle paint; does not cover tasks model/timer / subagent open chrome) or `/clear-completed-todos` |
| `Ctrl+G` | Agent screen (full TUI) | Toggle the tasks pane |
| `Ctrl+G` | Ordinary composer (minimal mode) | Edit the current draft in an external editor without sending it. If the terminal reserves this chord, choose **Edit Prompt in External Editor** from the command palette. |
| `Ctrl+L` | Agent screen | Open the extensions modal (**non–VS Code family only**; on VS Code / Cursor / Windsurf / Zed, `Ctrl+L` is mid-turn **interject** and extensions open via `/plugins` / `/hooks`) |
| `↑` | Prompt focused (empty prompt, normal input mode) | Open the history panel with your last prompt filled in; `↑`/`↓` step through entries (each lands in the input), `↓` at the newest closes the panel, and typing edits the recalled prompt in place. Recalled `!` shell commands re-enter shell mode. `↓` never opens history. |
| `!` | Prompt focused | Enter shell mode (type `!` on an empty prompt) |
| `Ctrl+.` (alt: `Ctrl+X`) | Agent screen | Open the keyboard shortcuts help |
| `F2` (alt: `Ctrl+,` / `Cmd+,`) | Agent screen | Open the settings modal |

**Note:** `Ctrl+M` is context-dependent. When the prompt is focused, it toggles multiline input mode. Otherwise, it opens the model picker.

**Note:** Minimal-mode external editing resolves `$VISUAL`, then `$EDITOR`, then `vi`. Values may include quoted arguments. Saving replaces only the draft; an empty file clears it. Drafts with pasted/file/image chips must be edited in the composer so attachments are not flattened.

**Note:** `Ctrl+'` is a Windows alt for `Ctrl+;` — some Windows consoles drop the `Ctrl` modifier on punctuation keys.

**Note:** `Ctrl+.` needs the Kitty keyboard protocol (or tmux `extended-keys on` so that protocol can pass through). On VS Code / Cursor / Windsurf / Zed integrated terminals, VTE, Apple Terminal, Windows Terminal, JetBrains, tmux with `extended-keys off`, screen, and similar no-KKP setups, Grok advertises **`Ctrl+X`** as the primary shortcuts-cheatsheet key instead. **`Ctrl+X` always works** as a classic control character even when `Ctrl+.` does not. Run `/doctor` if modified keys misbehave in tmux.

---

## Image Paste & Drag-and-Drop

| Action | macOS | Linux | Windows |
|---|---|---|---|
| Drag image from file manager into the prompt | Finder ✓ | Files / Dolphin ✓ | Explorer ✓ |
| Copy a file in the file manager, then paste | `Cmd+V` | `Ctrl+V` | `Ctrl+V` |
| Screenshot or "Copy Image" in clipboard, then paste | `Cmd+V` | `Ctrl+V` | **`Alt+V`** |

Non-image files insert their absolute path as text instead of a chip.

> **`Alt+V` on Windows** is grok-specific. Windows Terminal's default `Ctrl+V` only pastes plain text and silently drops image clipboards; `Alt+V` bypasses the interceptor. To use `Ctrl+V` for images too, add `{ "command": null, "keys": "ctrl+v" }` to `actions` in your Windows Terminal `settings.json`.

### Linux PRIMARY and CLIPBOARD

Linux X11 has two independent text selections:

- `Ctrl+V` reads **CLIPBOARD**, the explicit copy/cut selection. It never falls back to PRIMARY. To put text there with `xclip`, use `printf %s "text" | xclip -selection clipboard`.
- An unmodified middle click in Grok reads **PRIMARY**, the current mouse selection, only when `DISPLAY` is non-empty. Pure X11 can use its native reader fallback; XWayland requires `xclip` or `xsel` on `PATH` so Grok reads the X11 selection rather than Wayland PRIMARY. The press is handled once; the release does not paste again.
- `Shift+Insert` is the terminal-native way to paste selected text. Many terminals also use `Shift+middle click` to bypass application mouse reporting.

Over SSH, the remote Grok process usually cannot access the terminal's local X11 selection. Use terminal-native `Shift+Insert` or `Shift+middle click` so the local terminal sends the selected text through the PTY.

---

## During an active turn (agent running)

Plain `Enter` is **not always "send."** The shortcuts bar footer labels the real outcome (`Enter: send`, `Enter: queue`, or `Enter: interject`) using the same rules as dispatch. Read that label before you type if you are unsure.

While the agent is generating, or while the parent looks idle but background subagents still hold the queue:

- **Plain `Enter`** (with text in the composer) **queues** a follow-up for later. Queued follow-ups run after the current turn ends, and they deliberately **hold** while:
  - the agent is **blocked waiting** on background-task output or a foreground subagent, or
  - **any background subagent is still live** even when the parent looks idle.

  Status examples:
  - Before anything is queued: `N subagent(s) still running · Enter queues`
  - After items are held: `N subagent(s) still running · M queued — Interject to force`

  Running **monitors** alone do **not** hold the queue (they can run forever). When the last holding subagent finishes, the queue drains automatically.
- **`Enter` again on the emptied composer** (double-Enter) **soft-interjects** the **top** queued follow-up into the running turn (mid-turn only).
- The **interject** chord is **soft only** mid-turn: it injects your message into the **current** turn at the next safe point. It **never cancels** the running turn — cancel is **Esc / stop** only:
  - **Non-empty composer** → soft-interject that text into the running turn.
  - **Empty composer** + a queued follow-up → soft-interject the **top** queued follow-up (no need to focus the queue pane). On the queue pane, the same chord (or the **[Interject]** button) soft-interjects the **selected** row (plain prompts only; bash / non-plain stay queued).
  - **Idle with live background subagents** + held queue (or typed text) → force-start the next turn without waiting for children (nothing to cancel).
  - **Idle** with nothing held / nothing to interject → toast (never a silent no-op).
- While the agent is **blocked waiting** (on task output or a subagent) **and the queue is empty**, plain `Enter` with text **cancels the blocked wait and runs your message next** (shell auto `sendNow` / cancel-and-send). That is an **intentional unblock**, not soft Interject — soft Interject is for mid-turn steer only; cancel on a normal running turn is still **Esc / stop** only. If anything is already queued, plain Enter appends and holds like a normal mid-turn queue.
- While the parent is **idle with live background subagents**, plain `Enter` with text **queues and holds** (does not start a conflicting main turn). Use the **interject** chord to force, or wait for children to finish.

| Terminal | Primary | Alternates | Action |
|----------|---------|------------|--------|
| Default | `Ctrl+Enter` | `Ctrl+I` | Soft interject (inject into the running turn; does **not** cancel) |
| Apple Terminal | `Ctrl+O` | `Ctrl+Enter`, `Ctrl+I` | Soft interject |
| VS Code family (VS Code, Cursor, Windsurf, Zed) | **`Ctrl+L`** | *(none)* | Soft interject (`Ctrl+I` not used — Tab / host chat; plugins via `/plugins`) |

In `/multiline` mode, `Shift+Enter` (or `Alt+Enter`) sends while plain `Enter` inserts a newline — except on an **empty** composer mid-turn with a queued follow-up, where plain `Enter` still **soft-interjects** the top row (same as normal mode). (`Ctrl+Enter` is the interject chord mid-turn when bound on non–VS Code family; it does not submit a new idle turn.)

Interject is **not** cancel-and-send. To stop the turn, use **Esc** or the status-row **`[stop]`** control (hard cancel). To hand the agent a note for the **next** turn without steering mid-turn, queue with plain `Enter`; the agent picks it up at the next turn boundary.

### Pause vs stop (discoverable chrome)

Three different work controls exist. They are not interchangeable:

| Control | How to find it | What it does |
|---------|----------------|--------------|
| **Hard stop / cancel** | Status-row **`[stop]`** (red on hover), **2× Esc**, or empty-prompt **Ctrl+C** | Cancels the focused session’s current turn. When the parent is idle but subagents still run, the same cancel path can open the subagents panel or stop those subagents per preference. |
| **Global pause** | Status-row **`[pause]`** / **`[resume]`** (quiet white on hover), or **Ctrl+Shift+Space** | Pauses **all** sessions in this process: cancels in-flight turns, holds queues, then resumes only unfinished work. Not a media-player freeze of a single stream. |
| **Soft stop** | **Ctrl+Shift+S** only (no status-row button) | Lets the **current** turn finish, then holds the queue so nothing new starts until you disarm. |

The shortcuts bar also shows a **pause** (or **resume**) hint while work is live or global pause is holding sessions, next to cancel when a turn is running.

> **WezTerm**: These modified Enter keys need `enable_kitty_keyboard = true` in your WezTerm config. Full steps and a one-line workaround are in the [terminal support guide](21-terminal-support.md#problem-ctrlenter-doesnt-interject-in-wezterm).

> **Windows (non–VS Code family)**: Some consoles drop the `Ctrl` modifier on `Ctrl+Enter` (it can collapse to bare `Enter` or `Ctrl+J`). Use `Ctrl+I` as the alt — letter-key Ctrl chords are stable everywhere. On VS Code family, use **`Ctrl+L`**.

> **VS Code family `Ctrl+L`**: Grok uses it for interject and leaves the extensions shortcut unbound (open plugins with `/plugins` or the command palette). If your terminal profile still maps **Clear** (or another command) to `Ctrl+L`, that host binding can steal the chord — rebind or remove it so the PTY receives form feed (`\x0c`).

---

## Global

Actions available from any screen.

| Key | Alt Key | Action | Confirmation |
|-----|---------|--------|-------------|
| `Ctrl+N` | | Create a new session (optionally in a git worktree) | Yes (double-press within 1000ms) |
| `Ctrl+Q` | `Ctrl+D` | Quit the application | Yes (double-press within 1000ms) |
| `F9` | | Capture the current TUI frame as a PNG (`/screenshot`) | No |

**VS Code family terminal** (VS Code, Cursor, Windsurf, Zed integrated terminals): `Ctrl+Q` is captured by the host, so Grok makes **`Ctrl+D` the sole quit key** (`Ctrl+Q` is not bound). Half-page-down is rebound to bare **`Shift+D`**. Mid-turn interject uses **`Ctrl+L`** (no alternates) because `Ctrl+Enter` / `Ctrl+I` do not reliably reach the PTY; extensions are opened via `/plugins` instead of `Ctrl+L`.

> **Returning to the welcome screen has no key binding** — use the `/home` slash command (alias `/welcome`) from inside a session. See [Slash Commands](04-slash-commands.md).

### Destructive Action Confirmation

Actions marked with "Yes" in the confirmation column require a double-press within 1000ms. Press the key once to see a confirmation prompt, then press again to confirm. This prevents accidental session loss.

---

## Welcome Screen

Bindings that only fire on the welcome screen (before any agent session is open).

| Key | Action |
|-----|--------|
| `Ctrl+S` | Resume session (open the session picker) |
| `Ctrl+W` | Open the New Worktree dialog (only inside a git repository) |
| `Ctrl+I` | Import Claude settings (when available) |
| `Ctrl+Shift+I` | Dismiss the Claude import row (when available) |

`Ctrl+W`, `Ctrl+I`, and `Ctrl+Shift+I` are only active on the welcome screen. `Ctrl+S` opens the session picker on both the welcome screen and inside an agent session (where it opens as a modal overlay, same as the `/resume` command). `Ctrl+Q` is the same global Quit binding documented above, not a welcome-specific handler.

---

## Command Palette

Press `Ctrl+P` or `?` to open the command palette -- a searchable list of actions. The palette shows:

- All keyboard shortcuts with their current bindings
- All slash commands
- Available skills

Type to filter, then press `Enter` to execute the selected action.

---

## Shortcuts Bar

The bottom of the TUI displays a contextual shortcuts bar showing the most relevant key bindings for the current state. The hints change based on:

- Which pane is focused (scrollback vs. prompt)
- Whether the agent is currently running
- What type of entry is selected

---

## Mouse Support

The TUI supports mouse interaction:

- **Click** on a scrollback entry to select it
- **Click** always-on **`⧉`** on a user or assistant message bubble to copy that bubble without selecting it first (default on via `scrollback.display.bubble_copy_buttons`)
- **Scroll wheel** to scroll through the scrollback
- **Click** on the prompt area to focus it
- **Hover** over the prompt to see a highlight (configurable via `pager.toml`)
- **Middle click** on Linux X11/XWayland to paste the PRIMARY selection

---

## Quick Reference Card

### When scrollback is focused (Simple mode — default)

```
Navigation:       Up/Down (prev/next entry)  Shift+Left/Right (prev/next turn)
Scrolling:        Ctrl+J/K (line)  PgUp/PgDn (page)  Ctrl+U/D (half page)
Focus prompt:     Space or any letter key (auto-focuses and types)
```

### When scrollback is focused (Vim mode)

```
Navigation:       j/k (up/down)  H/L (prev/next turn)  K/J (prev/next response)  g/G (top/bottom)
Scrolling:        Ctrl+J/K (line)  Ctrl+U/D (half page; D=Shift+D in VSCode)  PgUp/PgDn (page)
Folding:          h/l (collapse/expand)  e (toggle)  E (all)
Content:          y (copy)  Y (copy cmd)  Enter (fullscreen)
View:             r (raw markdown)  Ctrl+E (thinking)
Focus prompt:     i, Tab, or Space
```

### When prompt is focused

```
Send:             Enter
Newline:          Shift+Enter or Alt+Enter
Multiline:        Ctrl+M (toggle)
Paste:            Ctrl+V (text, files, screenshots on macOS/Linux)
Selected text:    Middle click or Shift+Insert (Linux X11/XWayland PRIMARY)
Paste image:      Alt+V (Windows only — for screenshots / "Copy Image")
Copy draft:       click ⧉ on the prompt top border (full composer plain text)
Select all:       Cmd+A (macOS, Ghostty only — see note below)
Leave:            Tab (back to scrollback)
Cancel (running): Ctrl+C (empty prompt; non-empty draft clears first)
Clear (idle):     Esc Esc within 800ms (non-empty prompt)
Rewind (idle):    Esc Esc within 800ms (empty prompt + messages)
```

> **Cmd+A is gated to Ghostty.** Grok's in-app `Cmd+A` handler is only
> wired up when the detected terminal is Ghostty. Other terminals
> either swallow `Cmd+A` at the terminal layer (Apple Terminal, default
> iTerm2) or apply their own in-terminal "Select All" behaviour (Kitty,
> WezTerm). On a non-Ghostty terminal, the binding does nothing and the
> key falls through to the terminal's native behaviour.
>
> On Ghostty, add the one-line unbind to `~/.config/ghostty/config` so
> the keystroke reaches the running TUI:
>
> ```ini
> keybind = cmd+a=unbind
> ```
>
> After Ghostty reloads (it watches the config file), `Cmd+A` in the
> prompt selects every character in the prompt buffer, including pasted
> image chips. Image chips are always path-free (`[Image #N]`); the
> filepath (when known) appears only in the image preview overlay on
> hover or when the cursor is on/right after the chip.

### Always available

```
Command palette:  Ctrl+P or ?
Model picker:     Ctrl+M (from scrollback)
Cancel:           Ctrl+C (see Escape table)
Always-approve:   Ctrl+O (toggle YOLO)
New session:      Ctrl+N (press again, then choose normal/worktree)
Quit:             Ctrl+Q (or Ctrl+D in VSCode)
```
