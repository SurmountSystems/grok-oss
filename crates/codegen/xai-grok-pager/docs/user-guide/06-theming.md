# Theming and Appearance Customization

Grok Build draws all TUI colors from a central theme. You can switch themes while Grok is running, follow your operating system's light or dark appearance, and adjust scrollback layout, animations, and block styling through configuration files.

---

## Available Themes

Grok includes six built-in themes, plus an `auto` option that follows your system appearance:

| Theme | Config Names | Description | Truecolor Required |
|-------|-------------|-------------|--------------------|
| **DOGE** | `doge` | Pure black background (`#000000`), white text/lines (`#FFFFFF`), and only the eight classic pure ANSI primaries (channels `0` or `255` only). **Default theme.** Black is pixel-off on emissive displays (design intent only — no measured power claims). | No |
| **GrokNight** | `groknight`, `grok-night`, `dark` | Neutral dark base with a magenta accent. Survives quantization cleanly on 256-color and 16-color terminals. | No |
| **GrokDay** | `grokday`, `grok-day`, `light`, `day` | Light theme for bright terminal backgrounds. | No |
| **TokyoNight** | `tokyonight`, `tokyo-night`, `tokyo` | Dark, blue-tinted backgrounds from the Tokyo Night palette. Loses its character when quantized. | Yes |
| **RosePineMoon** | `rosepine`, `rose-pine`, `rosepine-moon`, `rose-pine-moon` | Muted dark palette with mauve accents, from the Rosé Pine family. | Yes |
| **OscuraMidnight** | `oscura`, `oscura-midnight` | Deep dark base with purple accents. | Yes |

Theme names are case-insensitive. The `auto` option (alias `system`) is documented under [Auto Theme (System Appearance)](#auto-theme-system-appearance).

### DOGE (pure 3-bit)

The **DOGE** theme uses exactly eight pure digital colours — Black, Red,
Green, Yellow, Blue, Magenta, Cyan, White — with channels only `0` or `255`
(no mid-gray or alpha as palette colours). Informal set mnemonic: **RGBCMYKW**
(set membership; index order is classic ANSI SGR name order, not that string
order).

| Config name | Notes |
|-------------|--------|
| `doge` | Only accepted id (case-insensitive) |

**Black (`#000000`)** is treated as true off for the canvas (OLED-friendly
design). **Bright** SGR names use the same pure values as normal (already
saturated). Muted UI is chromatic (yellow secondary chrome, cyan ambient meta),
not mid-gray. Pure blue is not used for long body text or thin UI chrome.

#### Grok OSS semantic roles (application layer)

Normative pure palette (hex + no gray/alpha as colours): Surmount specs
[`0001_DOGE.md`](https://github.com/SurmountSystems/specs/blob/main/0001_DOGE.md)
(v1.0.0). That spec allows products to define semantic roles (Clause 8 MAY).
Grok OSS maps roles as follows on DOGE:

| Colour | Role in Grok OSS |
|--------|------------------|
| **Green** | **Human** — user prompt pointer (`❯`), left accent rail (`┃`), **composer box caret**, session cursor (OSC 12), success marks, slash skills, links |
| **Magenta** | **Agent** — running activity, **lower-left throbbers** (tool spinner + still-running cue), model label, assistant/thinking chrome |
| **Yellow** | Dates, times, timers, other useful context / secondary chrome |
| **Cyan** | System tags, limits, credits, path and ambient meta (not the activity throbber) |
| **Red / Blue** | Avoid unless contextually useful (errors stay red) |
| **Gray / alpha** | Forbidden as theme palette colours |

**Human prompts** always show a one-column green left rail (`┃`), same geometry
as the idle Recap white tool rail, via the shared accent column. Recap idle
expanded still uses white (`accent_tool`); loading Recap uses animated yellow
secondary chrome (`gray` token paints yellow on DOGE), not gray paint.

**Do not confuse these three surfaces:**

| Surface | DOGE colour | Notes |
|---------|-------------|--------|
| Composer box caret | Human **green** | Software caret on the prompt; stays green after you type or move arrows |
| Lower-left activity throbber | Agent **magenta** | Tool-running spinner and “subagents still running” cue |
| Todo clear-finished icon (`[−]`, open board + finished) | Quiet secondary (hover stronger) | Not neon green always-on; never agent magenta; not painted when board hidden or nothing finished |

Project internal palette + role annex:
`doc/dev/specs/doge-pure-8-colour-2026-07-26.md`.

### Minimal Mode Has No Theming

**Minimal mode** (`--minimal`) always renders with a single fixed terminal-native palette and ignores the `theme` settings entirely (they still apply to the full TUI). Minimal draws directly on your terminal's own background, so it uses your terminal's default foreground/background plus its 16-color ANSI palette — the same colors `git` or `ls` use — which stays readable on any light or dark terminal profile without detection or configuration. `/theme` and the theme rows in `/settings` are unavailable in minimal mode.

Syntax highlighting in minimal mode does **not** switch between light and dark theme files (polarity detection is intentionally avoided). Near-gray tokens inherit the terminal default foreground; chromatic tokens use base ANSI accents (red/green/yellow/blue/magenta/cyan) so read-file output and fenced code stay legible on both light and dark profiles.

---

## Switching Themes

### In the TUI

Run the `/theme` slash command (alias `/t`) to open the theme picker. As you move through the list with the arrow keys, Grok previews each theme in real time. Press Enter to apply and save your choice, or press Escape to revert.

To switch without the picker, pass a name directly:

```
/theme tokyonight
```

Submitting `/theme` on its own -- without choosing from the picker -- cycles to the next theme.

### Via Config File

Set the theme in `~/.grok/config.toml`:

```toml
[ui]
theme = "tokyonight"
```

**Default theme is DOGE** when `theme` is unset (and when `theme = "auto"` maps to
dark mode). To switch back to the previous neutral dark default:

```toml
[ui]
theme = "groknight"
```

Or pick **GrokNight** in the `/theme` picker or under Appearance in `/settings`.

---

## Auto Theme (System Appearance)

Set `theme = "auto"` to have Grok follow your operating system's light/dark appearance and switch themes automatically:

```toml
[ui]
theme = "auto"
```

By default, dark mode maps to **DOGE** and light mode maps to **GrokDay**. Override either mapping with `auto_dark_theme` and `auto_light_theme`:

```toml
[ui]
theme = "auto"
auto_dark_theme = "tokyonight"
auto_light_theme = "grokday"
```

`theme = "system"` is an alias for `theme = "auto"`.

### How Detection Works

| Platform | Method |
|----------|--------|
| **macOS** | Reads `AppleInterfaceStyle` system preference |
| **Linux** | Queries XDG Desktop Portal (`org.freedesktop.appearance.color-scheme`) |
| **Windows** | Reads the system personalization registry |
| **SSH / headless** | Falls back to an OSC 11 terminal background query at startup |

Once running, Grok polls for appearance changes every 5 seconds. Toggling your OS between light and dark mode takes effect within seconds without restarting.

### Via the Settings Pane

Run `/settings` (alias `/config`) and open the **Appearance** category to set the **Auto dark theme** and **Auto light theme** interactively. Selecting `auto` in the `/theme` picker enables auto mode using these mappings.

---

## Color Support Detection

On startup, Grok detects your terminal's color capability level:

| Level | Description | Detection |
|-------|-------------|-----------|
| **Truecolor** (24-bit) | Full RGB color. All themes render as designed. | `COLORTERM=truecolor` or equivalent terminal capability |
| **256-color** | Indexed palette. RGB values are mapped to the nearest palette entry. | Standard xterm-256color |
| **16-color** | ANSI names only. Colors are mapped to the closest ANSI color. | Basic terminal support |

When you set `NO_COLOR`, Grok emits no color and renders in monochrome.

Run `/doctor` to see the detected color level and the themes available on this terminal. If truecolor is unavailable, Doctor shows the relevant setup steps or explains the terminal limitation.

### Automatic Quantization

Every theme is defined using full RGB values. At startup, Grok quantizes all colors to match the detected capability level. This means:

- On **truecolor** terminals, colors pass through unchanged.
- On **256-color** terminals, each RGB value is mapped to the nearest indexed palette entry.
- On **16-color** terminals, colors map to ANSI names.

GrokNight and GrokDay use neutral grays that quantize cleanly. TokyoNight, RosePineMoon, and OscuraMidnight use distinctive tinted backgrounds that lose their character when quantized, which is why the theme picker hides them on non-truecolor terminals.

### Runtime-Generated Colors

Colors generated at runtime (syntax highlighting, background blending) are also quantized through the same pipeline, ensuring consistent appearance across all terminal types.

---

## Cursor Color

Grok sets your terminal cursor to the current theme's `accent_user` color using the OSC 12 escape sequence, to indicate an active Grok session. The cursor color is:

- Applied on startup and on theme switch.
- Reset to the terminal's default on exit via OSC 112.

This works in terminals that support OSC 12 (most modern terminals).

### Composer box caret (software)

While the prompt is focused, Grok paints a **software box caret** (filled block
↔ empty cell blink) in the theme's **Human** accent (`accent_user` = pure green
on DOGE). That caret is the human input surface, not agent chrome: it must stay
Human green (not agent magenta).

- Typed letters under the caret keep their grapheme (reverse plate or green
  ink); the caret does not eat characters.
- When you move the caret (arrows, Home, End), **previous cells repaint as
  normal text**. There must be no leftover green plate, green letter ink, or
  solid block glyph stuck on letters behind the real caret.

Hardware terminal cursor is hidden while the software box caret paints, so you
do not see two cursors. OSC 12 still marks the session as Grok-owned for hosts
that show it when focus leaves the software caret path.

### Lower-left activity throbber

The lower-left turn-status area uses **agent magenta** (`accent_running` on
DOGE) for busy tool activity and for the idle “still running” concentric icon
when background subagents (or other watchers) keep work alive. That is not
system cyan and not Human green.

---

## Compact Mode

Toggle compact mode with the `/compact-mode` slash command. Compact mode:

- Removes outer vertical padding (top/bottom margins become 0).
- Reduces horizontal padding to the minimum (1 column).
- Reduces top padding in the prompt area and info blocks.

The setting is persisted in `~/.grok/config.toml` under `[ui].compact_mode` and survives restarts.

Use compact mode on small screens to maximize content area.

---

## Hide header

Hide **in-app** chrome headers (status / welcome / dashboard) to reclaim vertical
space. This is **not** the desktop or terminal **window title** (see
[Window title](#window-title) below).

```toml
[ui]
hide_header = true
```

Or toggle **Hide header** under Appearance in `/settings`. Default is off
(headers visible).

When enabled, the same knob zeros:

1. **Agent status bar** (cwd/git left; context %, queue badge, plan chip, todos right)
2. **Welcome location top bar** (git branch / cwd line on the welcome screen)
3. **Dashboard location header** (location + status chips + New Agent button row)

Hiding is intentional opt-in: you lose those clicks and labels. Minimal mode has different chrome and ignores this setting.

---

## Window title

Grok **always manages** the desktop or terminal tab/window title text (OSC
SetTitle) when dynamic titles are enabled. This is **not** the in-app agent
status bar and **not** the same as [Hide header](#hide-header).

Examples:

| Situation | Example title |
|-----------|----------------|
| Idle single session | `my-session - grok-oss` |
| Multi-agent busy | `Thinking - my-session - 2 agents - grok-oss` |

`N agents` appears only when more than one top-level agent is busy. Startup
always writes a product-managed title (session or `grok-oss`), never raw process
argv like `grok-oss --resume …`, and never an empty title (empty OSC blanks DE
switchers).

### Turn dynamic titles off

Opt out with nested notification title config (not a separate hide flag):

```toml
[ui.notifications.title]
enabled = false
```

That stops session/activity title updates. Product never blanks the window with
an empty SetTitle.

### Customize title items

When `title.enabled` is true (default), layout is controlled by `title.items`
(defaults include `session-name` and `agents`):

```toml
[ui.notifications.title]
enabled = true
items = ["action-required", "spinner", "activity", "session-name", "agents", "grok"]
```

The `grok` slot renders as **`grok-oss`**. Full options:
[Configuration → Notifications](05-configuration.md#notifications).

---

## Host window frame (edgeless)

Grok OSS is a **TUI inside your terminal emulator**. It can set **title text**
(OSC), but it **cannot** remove or restyle the OS / compositor window chrome
(close buttons, client-side decorations, borders). There is **no product flag**
that undresses arbitrary terminals, and Appearance settings do not own CSD.

If you want a borderless or “edgeless” look, configure the **host terminal**
(and sometimes the desktop). Copy-paste starters:

### kitty

In `kitty.conf`:

```conf
hide_window_decorations yes
# or: hide_window_decorations titlebar-only
```

Reload kitty config or restart the terminal.

### foot (Wayland)

In `foot.ini`:

```ini
[csd]
preferred=none
```

`server` keeps server-side decorations when the compositor provides them;
`client` uses foot’s own CSD. Behavior depends on your compositor.

### Ghostty

In Ghostty config (GTK builds; check your version’s docs if a key differs):

```conf
window-decoration = false
```

Some builds also accept `true` / `auto`. Confirm with `ghostty +show-config` or
upstream docs.

### GNOME and other desktops

Many GNOME apps draw client-side decorations. Hiding the frame is a **desktop +
terminal** choice (tiling WM, extensions, terminal prefs), not something
grok-oss can force. See [Terminal support](21-terminal-support.md) for which
hosts Grok detects.

---

## Syntax Highlighting

Grok bundles four `.tmTheme` files for code-block syntax highlighting and selects one based on the active theme:

- `grok-night.tmTheme` -- GrokNight, RosePineMoon, OscuraMidnight
- `doge.tmTheme` -- DOGE — pure 8-colour primaries only (`#000000` / `#FFFFFF` + the eight primaries)
- `grok-day.tmTheme` -- GrokDay
- `tokyo-night.tmTheme` -- TokyoNight

Grok selects the matching file automatically when you switch themes. The `.tmTheme` files are built into the binary, so you cannot replace them with your own.

Under **DOGE**, the context usage bar also uses **solid DOGE colour steps** (no mid-gray gradient) so usage urgency stays on the pure 8-colour palette.

---

## Deep Customization with pager.toml

For fine-grained control over the TUI appearance, create `~/.grok/pager.toml`. This file controls scrollback layout, block styling, animations, and more. All settings have defaults; specify only the values you override. (Dev builds generate this file as a template with every default commented out — uncomment a line to override it; commented values keep tracking future defaults.)

### Layout

Control viewport padding and block spacing:

```toml
[scrollback.layout]
outer_vpad = 1          # Vertical padding (top/bottom) for the viewport
outer_hpad_left = 2     # Left margin (minimum: 1)
outer_hpad_right = 2    # Right margin (minimum: 1)
block_pad_left = 2      # Padding between accent line and content
block_pad_right = 2     # Padding after content at right edge
```

### Scrollbar

```toml
[scrollback.scrollbar]
enabled = true          # Show/hide the scrollbar
gap_left = 0            # Gap between content and scrollbar (0 = adjacent)
gap_right = 0           # Gap between scrollbar and screen edge (0 = at edge)
# scrollbar_bg = "none" # Override background color (or "none" for theme default)
# scrollbar_fg = "none" # Override thumb color (or "none" for theme default)
```

### Scroll Behavior

```toml
[scrollback.scroll]
margin = 0                  # Context lines above/below selected entry (0 = edge)
min_page_fraction = 0       # Minimum scroll as % of viewport (0-100)
follow_indicator = "center" # "center" = show down-arrow, "none" = hidden
follow_auto_select = true   # Auto-select latest entry when following
follow_by_overscroll = true # Scrolling past bottom engages follow mode
anchor_on_fold = true       # Keep block header at same screen position when folding
```

### Display Options

```toml
[scrollback.display]
sticky_headers = true              # Pin user prompts as headers when scrolled past
tab_width = 4                      # Spaces per tab character (0 = pass through)
expandable_indicator = true        # Show "›" on foldable collapsed entries
expandable_indicator_char = "›"    # Character to use (default: "›")
collapsed_accent_char = "❙"        # Accent for collapsed groupable blocks (falls back to "|" on the legacy Windows console)
dim_accent = 0.5                   # Blend factor for dimmed accents (0.0-1.0)
line_under_last_entry = false      # Horizontal line below last entry
selection_buttons = true           # Show ⧉/↗ on selection box when a block is selected
bubble_copy_buttons = true         # Always-on ⧉ on user/assistant bubbles (no select-first)
```

### Animation

```toml
[animation]
fps = 30           # Frame rate (1-60). Higher = smoother, more CPU
wave_rows = 32     # Rows per wave cycle for accent animation
```

### Block Styling: Edit Diffs

```toml
[scrollback.blocks.edit]
indent = true                   # Indent diff content
vpad = false                    # Vertical padding around diffs
# expanded_by_default = true    # Unset: follows [ui] collapsed_edit_blocks in config.toml
                                # (flag on = collapsed one-liner); uncomment to pin either shape
hunk_separator = "…"            # Separator between hunks ("…", "───", "⋯", or "" for none)
dual_line_numbers = false       # Two-column line numbers (old + new, like GitHub)
# line_summary = false          # Show +N/-M in the collapsed header; unset follows the same flag
# bg = "none"                   # Block background ("none", "light", "dark")
```

### Block Styling: Thinking/Reasoning

```toml
[scrollback.blocks.thinking]
accent_enabled = true       # Show accent line for thinking blocks
animate = true              # Animate accent line while thinking
truncated_lines = 3         # Lines to show in truncated mode
bg_blend = 70               # Markdown-color blend with background (0-100)
header = true               # Show "Thinking..." header
header_bright = false       # Bright header style (vs dim/muted)
```

### Block Styling: Tool Calls

```toml
[scrollback.blocks.tool]
muted_collapsed = true     # Gray out collapsed tool calls
dim_details = true          # Dim parenthetical details (line counts, match counts)
bullet = "diamond"          # Bullet style before tool headers
```

Available bullet styles:

| Config Value | Character | Description |
|-------------|-----------|-------------|
| `none` | (none) | No bullet |
| `dot` | `·` | Middle dot (smallest) |
| `small-circle` | `•` | Bullet |
| `circle` | `●` | Filled circle |
| `small-triangle` | `▸` | Right-pointing small triangle |
| `triangle` | `▶` | Right-pointing triangle |
| `diamond` | `◆` | Filled diamond (default) |

### Block Styling: Execute (Shell Commands)

```toml
[scrollback.blocks.execute]
first_lines = 2                   # Output lines shown at start in truncated mode
last_lines = 3                    # Output lines shown at end in truncated mode
accent_enabled = true             # Show accent line (animated while running)
header_style = "label"            # "shell" ($ prefix) or "label" (Run prefix)
muted_command_collapsed = true    # Mute command text when collapsed
```

### Block Styling: User Prompts (Scrollback)

```toml
[scrollback.blocks.prompt]
vpad = true            # Vertical padding
bg = "light"           # Background ("none", "light", "dark")
show_prefix = true     # Show the prompt prefix character
min_lines = 2          # Minimum content lines in truncated/sticky mode
```

### Prompt Input Widget

```toml
[prompt]
collapse_unfocused = true    # Collapse when scrollback is focused
mouse_hover = true           # Show hover highlight on mouse over
show_prefix = true           # Show the prompt prefix character
```

### Todo Badges

```toml
[todo]
badge_format = "default"   # "default" = 2/5 (done/total), "colon" = [▶:1 □:4 ✓:3 ✗:2], "comma" = [1 ▶, 4 □, 3 ✓, 2 ✗]
```

### Terminal Behavior

```toml
[terminal]
alt_screen = "auto"    # "auto", "always", or "never"
```

Alt-screen policies:
- `auto` -- fullscreen in plain terminals and normal tmux; inline in tmux control mode and Zellij.
- `always` -- always enter fullscreen.
- `never` -- never enter fullscreen; run inline in the main scrollback.

### Plugins UI

```toml
disable_plugins = false   # Set to true to hide /hooks, /plugins commands and annotations
```

---

## Theme Color Slots

Each theme defines the following color slots that are used throughout the TUI:

**Backgrounds:** `bg_base`, `bg_light`, `bg_dark`, `bg_highlight`, `bg_hover`, `bg_terminal`, `bg_visual`

**Accents:** `accent_user`, `accent_assistant`, `accent_thinking`, `accent_tool`, `accent_system`, `accent_error`, `accent_success`, `accent_running`, `accent_skill`, `accent_plan`, `accent_verify`, `accent_feedback`, `accent_remember`, `accent_model`

**Text:** `text_primary`, `text_secondary`

**Grays:** `gray_dim`, `gray`, `gray_bright`

**Semantic:** `command`, `path`, `running`, `warning`, `fuzzy_accent`

**Borders and scrollbar:** `selection_border`, `hover_border`, `prompt_border`, `prompt_border_active`, `scrollbar_bg`, `scrollbar_fg`

**Paste:** `paste_bg`, `paste_fg`, `paste_dim`

**Diff:** `diff_delete_bg`, `diff_delete_fg`, `diff_insert_bg`, `diff_insert_fg`, `diff_equal_fg`, `diff_gutter_fg`

**Markdown:** heading colors (`md_heading_h1`-`md_heading_h6`), `md_code`, `md_code_bg`, `md_text`, `md_muted`, `md_task_checked`, `md_task_unchecked`, `link_fg`

The theme system manages these slots internally and quantizes them automatically for your terminal.
