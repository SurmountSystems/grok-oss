# Slash Commands

Type `/` in the prompt to open the command menu. It fuzzy-matches as you type, and picking a command runs it immediately.

Commands come from two places: **shell builtins**, handled by the agent backend (xai-grok-shell), and **pager builtins**, handled by the pager frontend (xai-grok-pager). Both show up in the same menu, and any enabled skill with `user-invocable: true` appears there too. If a skill reuses a built-in name such as `login`, the built-in keeps `/login` and the skill stays available as `/plugin-name:login` — the menu badges both so the collision is visible.

Every command below lists its aliases where it has them. A few commands only appear when a feature or session state enables them; those cases are called out inline. The menu is also filtered by render mode — see [`/minimal` and `/fullscreen`](#minimal-and-fullscreen).

---

## Session Management

### `/new`

Start a fresh session and clear the current conversation. Alias: `/clear`.

### `/resume`

Open the session picker to reload a previous session from disk.

### `/start`

Start paused or interrupted work in the current session. If every session in this process is globally paused, `/start` unpauses and continues the interrupted turns. If this session has a continue-interrupted marker (`canceled_turn_resume.json`), `/start` re-queues that prompt once. If a soft-stop hold is keeping the queue from draining, `/start` releases that hold. If nothing is paused or interrupted, it says so and does not start a new turn.

`/start` is not `/resume`. `/resume` only opens the session picker.

### `/dashboard`

Open the [Agent Dashboard](23-dashboard.md): live roster of top-level sessions in this pager (peek, reply, dispatch, pin, rename, stop, attach). Aliases: `/agents-dashboard`, `/sessions`.

Not `/config-agents` (alias `/agents`), which manages agent *definitions* and personas. Not `/running` (alias `/windows`), which lists live grok-oss TUI windows on this machine. Hidden in minimal mode; disable with `GROK_AGENT_DASHBOARD=0` or `[dashboard].enabled = false`.

### `/running`

List live grok-oss TUI windows on this machine. Alias: `/windows`.

This is **Running grok-oss sessions**. It is not the [Agent Dashboard](23-dashboard.md), not `/sessions`, not `/tasks`, not `/resume`, and not `/start`. `/dashboard` still owns the roster inside this pager process. Do not treat `/running` as a second dashboard.

The list comes from `$GROK_HOME/active_sessions.json`. When `GROK_HOME` is unset, that file is `~/.grok/active_sessions.json`. Two grok homes do not see each other. Only live grok-oss processes appear. Two windows on the same conversation both appear. The row for this TUI is marked `(this window)`.

Each row shows the PID, a short session id, the working directory, when the window opened, and activity `working`, `idle`, or `unknown`. A title, when present, comes from the on-disk session summary, not from the latest user prompt. A short activity line may name the model, say turn running or paused, or give a subagent count. A live sibling with no heartbeat (an older binary) is `unknown`. That is honest, not fake idle.

The registry never stores prompts, tool arguments, tokens, JWTs, file contents, or message text.

The report is a transcript table. It refreshes when you run `/running`. It does not keep appending on a timer.

Default headless processes stay unlisted unless `GROK_TRACK_HEADLESS` is already set. Leader daemons stay on `grok-oss leader list`.

From a shell, the same filtered list is:

```
grok-oss running
grok-oss running --json
```

The human table uses the same columns. The CLI table does not mark this window, because there is no TUI window. `--json` is the same filtered rows and the same safe fields only (`pid`, `session_id`, `cwd`, `opened_at`, `updated_at`, `activity`, `title`, `activity_line`).

### `/compact [context]`

Compress conversation history to reclaim context-window space. Pass a note to tell Grok what to keep. Alias: `/compaction`.

Immediate `/compact` or `/compaction` still runs compact when the session is idle. To put compact on the existing composer prompt queue without running it this turn, use first-arg `queue` or `later`, or `/queue /compaction`. That is the same prompt queue as ordinary follow-ups, not a second queue. A cancelled compact still does not re-arm on the same turn.

```
/compact
/compaction
/compact keep the auth implementation details
/compact queue
/queue /compaction
```

Grok also auto-compacts once the context window hits **95%** by default (tune it with `/settings` → **Auto-compact at**, or `[session] auto_compact_threshold_percent`). Percent is of the *effective* sampling window AUTO uses. With **Economic mode** on (default), that sampling window is soft-capped at 200k tokens even when the model catalog is larger (for example 500k). The footer context chip then names both windows (`used / 200K sampling · 500K catalog`) so catalog 500k is not implied as the AUTO gate.

### `/queue`

With no args, list the composer prompt queue as a transcript block.

With a slash, hold that command on the **same** prompt queue so it does not run this turn. Supported holds: `/compaction` (and `/compact`), `/plan`, `/reports`, `/finish`. First-arg `queue` or `later` on those commands does the same hold.

```
/queue
/queue /compaction
/queue /plan
/queue /reports
/queue /finish
```

### `/economic-mode`

Cap (or uncap) effective context at 200k tokens for cheaper Grok 4.5 pricing. Default **on** for new sessions (`[ui] economic_mode` in `/settings`). Soft-caps the sampling window AUTO compact uses. When that sampling window is smaller than the catalog window, the footer context chip names both.

Token Economy may rewrite **implement-loop effort** (thoroughness 1–5, not model reasoning effort `/effort`, and not how many Review rows to launch) on `/implement`. One reviewer unless you explicitly asked for more. Optional lock and min floor always apply when set. Economic mode plus the cap master still own the hard ceiling (default 3) and desired inject when missing (default 2). See [Configuration → Token Economy](05-configuration.md#token-economy).

```
/economic-mode              # toggle this conversation
/economic-mode on|off       # set this conversation
/economic-mode status       # show session state
/economic-mode global on|off  # session + persist [ui].economic_mode
```

Aliases: `/economic`, `/econ`

### `/context`

Show how the context window is being used: a category breakdown (system prompt, messages, reasoning and overhead, free space) plus informational rows for tool definitions, the skills listing, and MCP server announcements with their estimated token cost.

### `/recap`

Generate a short "where was I" summary of the session so far. Alias: `/summarize`. The summary is display-only (not added to the model conversation). Grok OSS may also request the same kind of recap automatically when you return after being away.

**Default on.** Search `/settings` for `recap` to toggle auto return-from-away (`[ui.notifications] session_recap`), the debounce (`session_recap_threshold_secs`, default 30), and the master feature (`[features] session_recap`, restart required). `GROK_SESSION_RECAP=0` kills both `/recap` and auto recap.

### `/session-info`

Show session details — auth method, model, turn count, and context usage. Aliases: `/status`, `/info`.

### `/finish`

Write a structured post-mortem for this session. Work continues. A wrap often reveals more features worth adding. Leftover and next features stay first-class. Optional focus text is passed through to the agent. The product is not finished forever.

This is **not** `/dream` (memory consolidation), **not** `/recap` (a short chat recap), and **not** `/reports` (a checkpoint while work continues). `/finish` asks the agent to document what shipped, leftover, and useful next features. The artifact is a markdown file under `~/.agents/reports/` named `finish-YYYY-MM-DD.md` (or `finish-YYYY-MM-DD-<short-session>.md` if that dated name already exists). Complete American English. No secrets.

The host skill lives at `~/.agents/skills/finish/SKILL.md`. The pager builtin `/finish` keeps that slash name; a same-named skill cannot steal the bare command.

Immediate `/finish` injects that skill now. To hold it on the existing composer prompt queue, use first-arg `queue` or `later`, or `/queue /finish`.

```
/finish
/finish pager slashes and the ULID map
/finish queue
/queue /finish
```

### `/reports`

Write a checkpoint while work continues: what landed so far, leftover, and useful next features. This is not a wrap that says the project is done.

This is **not** `/finish` (session post-mortem), **not** `/dream` (memory consolidation), and **not** `/recap` (a short chat recap). The artifact is a markdown file under `~/.agents/reports/` named `reports-YYYY-MM-DD.md` (or `reports-YYYY-MM-DD-<short-session>.md` if that dated name already exists). Complete American English. No secrets.

The host skill lives at `~/.agents/skills/reports/SKILL.md`. The pager builtin `/reports` keeps that slash name; a same-named skill cannot steal the bare command.

Immediate `/reports` injects that skill now. To hold it on the existing composer prompt queue, use first-arg `queue` or `later`, or `/queue /reports`.

```
/reports
/reports pager slashes
/reports queue
/queue /reports
```

### `/polish`

Run a polish pass: make the product work well. This is a **default Grok OSS skill**. New grok-oss users get it without adding a project pack. Grok installs it from the product tree (`crates/codegen/xai-grok-bundle/skills/polish/`) into `~/.grok/bundled/skills/polish/` on startup. It is not a pager builtin and not a host overlay skill. It is not a project skill at `.agents/skills/polish/`.

This is **not** `/finish` (session post-mortem) and **not** `/reports` (a checkpoint while work continues). Type `/polish` to load the skill. Optional focus text is passed through.

```
/polish
/polish compact occupancy
```

### `/subagent`

Spawn one L2 coordinator for this job. The L1 main thread does not do the job. This is a **default Grok OSS skill**. New grok-oss users get it without adding a project pack. Grok installs it from the product tree (`crates/codegen/xai-grok-bundle/skills/subagent/`) into `~/.grok/bundled/skills/subagent/` on startup. It is not a pager builtin and not a host overlay skill. It is not a project skill at `.agents/skills/subagent/`.

Type `/subagent this ...` or `/subagent ...`. The rest of the line is the job passed to that L2 as a self-contained prompt.

This is **not** `/polish` (a polish pass) and **not** `/implement` (plan handoff). It is not the Hierarchical fast path on L1.

```
/subagent this diagnose the compact occupancy
/subagent implement the remaining-work pointer
```

### `/what`

Restate this session when you cannot parse the last agent chat. Not an apology. The agent replies with four labeled complete thoughts only: **What we are doing**, **What is true right now**, **What you need to do** (or `nothing`), **What I will do next**. Optional focus text is passed through. Follow Concise American Technical English as specified in Surmount `0005_CATE.md`.

This is **not** `/recap` (a short chat recap), **not** `/finish` (session post-mortem), and **not** `/reports` (a checkpoint file). Complete American English thoughts. No leftover board ids as the body.

`/what` is a **default Grok OSS skill** (in-tree `crates/codegen/xai-grok-bundle/skills/what`, installed into `~/.grok/bundled/skills/what`; not host overlay as the grok-oss source, not a pager-only prompt, not a project `.agents/skills/what` pack). The pager builtin `/what` keeps that slash name; a same-named skill cannot steal the bare command. Immediate `/what` injects that skill now. When you ask to revise a skill in grok-oss, edit `crates/codegen/xai-grok-bundle/skills/`. The live cache is not the source.

```
/what
/what the last status
```

### `/metadata`

Show live session context: grok-oss ULID, Grok Build / ACP UUID, working directory, model, when this window started, and this process id. Fields that are not known are omitted rather than invented. `/settings` **ULID session ids** (default on) chooses which id is listed first. The map still exists when that toggle is off. Not `/session-info` (auth, turn count, and context usage).

### `/fork`

Branch the current session into a new agent, keeping history up to this point.

### `/rewind` (alias: `/undo`)

Roll the conversation back to an earlier turn and discard everything after it. `/undo` is the same command.

### `/edit-prompt`

In minimal mode, open an external editor for an empty composer. Grok resolves `$VISUAL`, then `$EDITOR`, then `vi`; command values may include quoted arguments. Saving replaces the draft without sending it, and saving an empty file clears it. The command is hidden outside minimal mode.

```
/edit-prompt
```

To edit an **existing** draft when a terminal or multiplexer reserves `Ctrl+G`, open the command palette and select **Edit Prompt in External Editor**. That direct route preserves the existing text and refuses pasted, file-reference, or image chips without flattening them. Typing `/edit-prompt` into the composer necessarily replaces that input, so it starts from an empty draft.

### `/copy`

Copy the most recent response to the clipboard. Pass a number to copy the Nth-latest response instead, or a file path to write the text to a file rather than the clipboard (handy over SSH, where the local clipboard is often unreachable).

```
/copy
/copy 2
/copy out.txt
/copy 2 ~/exports/last-reply.md
```

Every copy is also written to a backup file — `~/.grok/last-copy.txt` by default, or `GROK_COPY_FILE` if set. Confirmed copies toast briefly (e.g. `Copied!`). Unverified OSC 52 deliveries and clipboard-unreachable fallbacks name the backup path so you can recover the text.

### `/export`

Export the conversation to a file or the clipboard.

### `/quit`

Quit the application. Alias: `/exit`.

### `/home`

Leave the current session and return to the welcome screen. Alias: `/welcome`.

### `/delete`

Delete the current session's history. Confirms first. Stops any running turn, background tasks, and subagents before wiping history. Returns to the welcome screen, or to the dashboard when you opened the session from the dashboard.

To delete a session you are not in, open `/resume` or the welcome session list and press `d` then `y`. On the dashboard, press `Ctrl+X` twice or click `[✗]`.

### `/rename`

Rename the current session. Alias: `/title`.

```
/rename new session title
/rename --auto
```

`--auto` unpins a manual title and lets auto-titling resume. It applies to Build sessions only — chat conversations have no local auto-titler. It must be the only argument (`/rename --auto Something` is an error). A session cannot be named `--auto` via this command; use the dashboard rename editor (`Ctrl+R`) for that pathological case.

### `/clear-completed-todos`

Archive completed and cancelled rows off the live session board. Pending and in-progress items stay. This is the same action as the compact **`[−]`** (U+2212 minus) in the todo header when the board is open and finished rows exist, and as optional focused `X` on the todo pane. Hints still say **Clear finished**. This is not `h` hide-done (that only hides finished rows on screen). This is not a `merge: false` wipe of open work.

```
/clear-completed-todos
```

---

## Model and Mode

### `/model <name>`

Switch models. Accepts a model ID or display name (case-insensitive), and for reasoning models you can add an effort level as a second argument. Alias: `/m`.

```
/model grok-build
/model Grok Build
/model Reasoning X high
```

### `/effort <level>`

Set reasoning effort on the **current** model without reselecting it. Levels are `low`, `medium`, `high`, and `xhigh`, and it only applies when the active model supports reasoning effort.

```
/effort high
```

### `/always-approve`, `/auto`, and `/context-only`

These are real toggles for the permission mode: they stay in the menu, and running the mode you're already in turns it back off.

| Command | When off | When already on |
|---|---|---|
| `/always-approve` | Skip all permission prompts | Back to ask |
| `/auto` | Classifier approves safe tools (dangerous ones may still prompt) | Back to ask |
| `/context-only` | Advertise no tools; refuse any tool call. Chat stays a conversation (redteaming / harness diagnosis) | Back to ask |

Running one while another is active switches modes. For example, `/auto` while always-approve is on switches to auto. `/auto` only appears when the auto permission-mode feature is enabled. `/context-only` is always offered. You can also change mode with `/settings`. `Shift+Tab` still cycles Normal / Plan / Auto / Always-approve; it does not include context-only. `Ctrl+O` still toggles always-approve.

Always-approve remains the preferred daily autonomy mode. Context-only is an explicit diagnostic mode, not the default.

### `/multiline`

Toggle multiline input. When it's on, `Enter` inserts a newline and `Shift+Enter` (or `Alt+Enter`) sends the message. Mid-turn, a bare `Enter` on an empty composer still force-sends the top queued follow-up. Alias: `/ml`.

### `/history`

Open prompt-history search: fuzzy-search this session's prompts newest-first, then press `Enter` or `Tab` to drop a match back into the prompt.

For quick recall, press `↑` on an empty prompt instead. The panel opens with your most recent prompt already filled in; `↑`/`↓` step through entries (each lands in the input), `↓` past the newest entry closes the panel, and typing edits the recalled prompt in place.

### `/compact-mode`

Toggle compact display — less padding and tighter spacing for denser output.

### `/vim-mode`

Toggle vim-style scrollback keys (`j`/`k`, `h`/`l`, `g`/`G`, `y`/`Y`, and so on). With it off (the default), a bare letter or `Shift+letter` in the scrollback just focuses the prompt and types the character. The setting persists to `[ui] vim_mode`.

### `/minimal` and `/fullscreen`

Reopen the current session in the other render mode. `/minimal` (offered while you're in fullscreen) switches to the experimental scrollback-native mode; `/fullscreen` (offered while you're in minimal; alias `/full`) switches back to standard fullscreen mode. Both relaunch the pager on the same conversation for this session only — they don't touch `config.toml`, and the relaunch banner reminds you how to switch back. The `--minimal` / `--fullscreen` CLI flags are session-scoped the same way. To make plain `grok-oss` open in a given mode by default, use `/settings` → **Default screen mode** or set `[ui] screen_mode`.

A handful of commands only work in one of the two modes, because the surface they drive doesn't exist in the other: `/find`, `/jump`, `/timeline`, `/theme`, `/tutorial`, `/workflows`, and `/dashboard` are fullscreen-only, while `/expand` and `/edit-prompt` are minimal-only. Those are hidden from the command menu and the palette in the mode they can't run in. If you type one out anyway, Grok says why — and points you at whichever is actually useful. When the other mode is the only way to get it, that's the mode switch: `/theme isn't available in minimal mode (minimal renders with your terminal's own palette). Run /fullscreen to switch this session.` When this mode already does the job another way, it names that instead: `/expand isn't available in fullscreen mode — press Tab to focus the scrollback, then → on the block.` Everything else works in both. Note that `--no-alt-screen` still counts as fullscreen here, so it keeps the fullscreen-only commands.

### `/plan`

Enter plan mode. Immediate `/plan` (optionally with a description) still enters plan mode when you want it now.

To schedule plan mode on the existing composer prompt queue without entering it this turn, use first-arg `queue` or `later`, or `/queue /plan`. That is the same prompt queue as ordinary follow-ups, not a second queue. Present is not Approve. Empty Enter never Approves.

```
/plan [description]
/plan queue
/queue /plan
```

### `/view-plan`

Open the current saved plan in the right pane. The pane uses the same four idle actions as a live present: **Approve**, **Comment**, **Revise**, **Exit**. Copy, search, and Esc stay available. If grok-oss.db has an explicit recorded choice for this session, a dot marks that option. Clicking Approve is a real Approve only while a live waiter is parked; after Approve or Exit it does not re-arm Plan ready. Aliases: `/show-plan`, `/plan-view`.

---

## Memory

`/flush`, `/dream`, and `/memory` require memory to be enabled (`--experimental-memory` or `GROK_MEMORY=1`); `/memory` also needs a configured memory backend. `/remember` is always available.

### `/memory`

Browse, view, and manage saved memories. Pass `on` or `off` to enable or disable memory. Alias: `/mem`.

```
/memory
/memory off
```

### `/flush`

Save the current session's knowledge to memory right now, triggering an LLM summary of the most important content. Reach for it before compaction, or any time you want to lock in context.

### `/dream`

Run memory consolidation — merge session logs into organized topics.

### `/remember`

Save a note to memory immediately, without waiting for an automatic summary.

```
/remember the staging deploy uses the eu-west cluster
```

---

## Hooks and Plugins

`/hooks`, `/plugins`, `/marketplace`, and `/skills` all open the same extensions modal, each on its own tab.

### `/hooks`

Open the extensions modal on the Hooks tab, where you can view loaded hooks, add or remove custom ones, and toggle them individually. The modal does not grant project trust — see [10-hooks.md](10-hooks.md) for the trust model.

The shell also advertises individual `/hooks-list`, `/hooks-trust`, `/hooks-add`, `/hooks-remove`, and `/hooks-untrust` commands; in the pager these are folded into the `/hooks` modal.

### `/plugins`

Open the extensions modal on the Plugins tab to view installed plugins, install new ones from the marketplace, and manage trust.

The shell additionally supports subcommands (`/plugins list`, `/plugins install <source>`, `/plugins uninstall <name>`, `/plugins update`, `/plugins reload`). In the pager, the modal does the same work visually.

### `/marketplace`

Open the extensions modal on the Marketplace tab to browse and install plugins.

### `/skills`

Open the extensions modal on the Skills tab to view installed skills.

---

## Media Generation

### `/imagine <description>`

Generate an image from a text description.

```
/imagine a golden sunset over a calm ocean with silhouetted palm trees
```

### `/imagine-video <description>`

Generate a video from a text (or image) description. It plans shots, generates source images, and animates them with `image_to_video`.

```
/imagine-video a cat playing piano in a jazz club
```

---

## Scheduling

### `/loop [interval] <prompt>`

Run a prompt on a recurring interval. Give the interval as `30m`, `1 hour`, or `every 2 days`; leave it out and Grok will ask.

```
/loop 30m check deploy status
/loop check deploy status every hour
```

Intervals are `Ns` (seconds, minimum 60), `Nm` (minutes), `Nh` (hours), or `Nd` (days); anything under 60 seconds is raised to the minimum. Recurring tasks expire after 7 days, and you can cancel one with `scheduler_delete` using the job ID reported when the loop is created.

---

## Workflows and Goals

### `/goal`

Set, manage, or check an autonomous goal. Grok works across rounds and only marks the goal complete after an independent evidence review confirms the claim; if that review can't reproduce the result or has no usable evidence, the goal stays active or pauses with concrete gaps.

```
/goal Migrate the auth module to the new API
/goal status
/goal pause
/goal resume
/goal clear
```

Arguments are `<objective> [--budget <tokens>]`, or one of `status`, `pause`, `resume`, `clear`. The `--budget` here is a **token** budget for the goal run, separate from the agent-count budgets that workflows use. `/goal` appears when goal mode is enabled for the session. Which driver runs it depends on background workflows: with them on, the host evaluates each model round and runs adversarial verification on completion candidates; with them off, the legacy model-facing `update_goal` path reports progress and triggers verification.

### `/deep-research <query>`

Kick off a background research workflow. It plans a bounded set of questions, gathers structured claims with source evidence, cross-checks each claim on an independent verifier shard, and renders only the claims that survive, with their verified source locators. Failed shards, dropped claims, and researcher uncertainties are reported as coverage limitations, and the report is marked **Partial** whenever any remain.

```
/deep-research Compare the migration risks of PostgreSQL 17 and MySQL 9
```

The command returns right away — follow progress in `/workflows`, and the final report appears in the conversation on its own.

Model-launched workflows may set `agent_budget` on the `workflow` tool. It's an absolute cumulative cap on logical child-agent calls: every `agent()` call and every item in a `parallel()` panel spends one slot, while schema-correction retries don't. The default is 128, explicit values run 1–1,024, and a panel that would cross the remaining budget is rejected before any of its children launch. Separately, a host-configured cap (32 by default) bounds how many children run at a time per run; larger panels queue and still act as a barrier. `budget()` reports the cap as `total`, admitted calls as `spent`, `reserved` (always zero), and `remaining`. Named slash launches use the default budget.

### `/workflow`

Launch a saved workflow, or manage a running one by the session-unique display name shown in `/workflows`. Launch the same workflow twice and the display names are numbered (`review-changes`, `review-changes-2`); you never need the internal run IDs.

```
/workflow review-changes {"target":"origin/main...HEAD"}
/workflow pause review-changes
/workflow resume review-changes
/workflow stop review-changes-2
/workflow save review-changes
```

Project workflows live in `.grok/workflows/*.rhai`; user workflows live in `~/.grok/workflows/*.rhai`. A same-process pause/resume continues the original immutable script, args, and `agent_budget` cap from committed host-call results — to iterate, edit the returned script copy and launch it as a new run.

A budget-limited run is different: it only resumes through a model/tool resume request that supplies an `agent_budget` above the admitted agent count. A bare `/workflow resume <name>` can't raise the cap, so it rejects budget-limited runs. Runs interrupted by a process restart aren't resumed at all, because external effects have no stable cross-process identity. And resume is not exactly-once: an external effect whose result wasn't committed before a same-process pause can run again.

### `/workflows`

Open the live workflows **run** dashboard — active and retained runs, not a catalog of saved definitions. Each row shows the run's display name, phase, agent roster, progress, and result. Inside a run's detail view, `p` pauses, `r` resumes an ordinary pause, and `x` stops. Budget-limited runs can't bare-resume: `r` returns the shell's rejection (raise the cap with a model/tool resume that passes a higher `agent_budget`), while `x` still stops. `s` saves the run's script, but it's hidden for known built-ins and numbered duplicate handles — for those, choose a new unique `meta.name` and save the edited script explicitly.

---

## Other

### `/theme`

Switch the color theme. Alias: `/t`.

### `/feedback [message]`

Report an issue or send feedback. A message sends immediately. With none, a pane opens for a longer report: `Enter` sends, `Esc` discards.

```
/feedback
/feedback Something isn't working correctly
```

### `/btw`

Send an aside to the agent without interrupting the current task. The side question and its answer are not part of the main turn.

In the full TUI, a finished answer opens a **Done** panel:

- **`y`** (when the panel is focused) copies the full thread (`/btw <question>` plus the complete rendered answer, not only what is on screen).
- **`a`** opens a follow-up composer in the same btw session.
- **`Esc`** dismisses the panel.

In minimal mode (`--minimal`), the answer shows up in a dismissible panel above the prompt: `Esc` dismisses it, a finished answer is saved into native scrollback, and a late reply to an already-dismissed panel is dropped.

```
/btw also check the error handling
```

### `/note`

Leave a mid-session operator note that is **not** a pending main-turn prompt.

```
/note check queue hold when subagents finish
/note                  # list notes for this session
```

Bare `/note` (or `/notes`) lists notes. This does not call the model and does not touch the prompt queue.

### `/screenshot`

Capture the current Grok OSS TUI frame as a PNG under `$GROK_HOME/screenshots/tui-*.png`. Toast shows the path. This is not an OS screenshot of other windows.

**F9** is the same action. When plan approval is open, the capture **auto-attaches** to the plan composer so Approve / Revise / Clarify can send it. See [Plan Mode](19-plan-mode.md).

```
/screenshot
```

### `/mcps`

Open the MCP servers management modal.

### `/doctor`

Check the current session for terminal, clipboard, color, input, notification, and sandbox issues. Doctor shows what it found and how to resolve each issue. Run `/doctor fix` to list available automatic fixes; other findings include manual steps. `/terminal-setup`, `/terminal-check`, and `/terminal-info` remain aliases.

The dual-auth block also lists SuperGrok principal(s) (role plus fingerprint only) and console key fingerprints. See [Authentication](02-authentication.md).

### `/rebuild`

Rebuild this checkout's `grok-oss` binary and gracefully relaunch live instances on this machine. Not SpaceXAI download, and not worktree database rebuild.

1. Finds a Grok OSS source tree (`justfile` plus `crates/codegen/xai-grok-pager-bin`).
2. Copies the current installed `grok-oss` binary, when it exists, to a sibling file named `grok-oss.prev` next to it (under `${CARGO_HOME:-$HOME/.cargo}/bin/`).
3. Compiles from the git index (staged files). Unstaged working-tree edits are not part of that compile. Then runs `just install` (or a fixed cargo install when `just` is missing).
4. Verifies package version plus git SHA.
5. Signals other live grok-oss TUIs so they re-exec onto the new binary with the same session. Stock `grok` is not signaled. After two windows can share one conversation, rebuild still signals each live grok-oss PID once (dedupe by PID).
6. Re-execs this TUI. Mid-turn work uses continue interrupted turn (`canceled_turn_resume.json`), not invent success. Nested agents resume the same way a network disconnect does, and the Subagents list must not go empty. Ctrl-C quits and does not re-exec peers.

To roll back after a successful install, copy `${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss.prev` over `${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss` and make that file executable. That sibling file is the previous grok-oss binary from the last `/rebuild` that found an existing install.

CLI: `grok-oss rebuild`. Freshness only: `grok-oss update --check` (compare to Surmount `main`; no auto-install).

### `/release-notes`

View release notes for the current version. Alias: `/changelog`.

### `/docs`

Browse the built-in How-to Guides, open the online Build docs, or jump straight to a guide by title. Aliases: `/howto`, `/guides`.

```
/docs
/docs web
/docs Getting Started
```

- Bare `/docs` (or `/docs how-to`) opens the How-to Guides picker.
- `/docs web` opens https://docs.x.ai/build/overview in your browser.
- `/docs <title>` opens a specific guide by case-insensitive title match.

### `/tutorial`

Open the onboarding tutorial: a short list of topics (your first prompt, attaching context, navigation, slash commands, worktrees, plan mode, customization, switching from another agent tool) — each a ~30-second read, with `→` flowing straight to the next topic. Nothing auto-shows — this command (or the command palette) is the way in.

```
/tutorial
```

Aliases: `/tour`, `/onboarding`

### `/import-claude`

Open the Claude import modal to bring over `~/.claude` settings: permissions, environment variables, MCP servers, hooks, and paths.

---

## Agents and Personas

### `/config-agents`

Open the agents modal to view and manage agent definitions, set the default, and switch the active one. Alias: `/agents`.

Not the live multi-session [Agent Dashboard](23-dashboard.md) (`/dashboard` / `Ctrl+\`).

### `/personas`

Create, edit, and delete personas. A subagent can apply a persona to shape how it behaves.

---

## Account and Billing

### `/login`

Log in or re-authenticate without leaving the session.

A second SuperGrok plan is visible only after a second `grok-oss login` that stores the Team principal. grok.com's account switcher is a different product. The second login does not wipe the first stored SuperGrok login. See [Authentication](02-authentication.md#included-supergrok-period-limits-and-limits).

### `/logout`

Log out and return to the login screen.

### `/usage`

View **session** token and cost totals, then SuperGrok billing when the consumer surface is visible. Alias: `/cost`.

When included SuperGrok period bounds are known, also shows **linear-burn pacing** (ahead of or behind linear burn for the billing period; never as dollars). Full double-entry books are on `/spend` and a section of `/limits`.

```
/usage
/usage manage
```

### `/spend`

Token Economy double-entry: local calculated spend (from session `usage.jsonl` ingested into `$GROK_HOME/grok_oss.db`) next to remote Management samples when a management key is available. Shows gap honesty when local cost ticks are missing. Meters stay distinct (included SuperGrok period limits ≠ SuperGrok dollar credits ≠ console team prepaid). Aliases: `/double-entry`, `/ledger`.

```
/spend
```

### `/limits`

Opens a dismissible popup (Esc to close) with spend meters from cached billing, not session tokens. Same data as clicking the compact meter on the top status row. CLI: `grok-oss limits` and `grok-oss limits --json`.

Keeps each meter distinct:

- **Included SuperGrok period limits** (used % · remaining % · next reset). SuperGrok is paid. This is the subscription-included quota for the current SuperGrok billing period, not SuperGrok dollar credits.
- Linear-burn pacing when period bounds exist (ahead of or behind linear burn; omit when bounds are missing).
- SuperGrok **dollar credits** (prepaid top-ups; separate from included SuperGrok period limits).
- **Console API key** request path and **console team prepaid** when a Management key and `[endpoints] management_team_id` (or `XAI_MANAGEMENT_TEAM_ID`) are set. Honest gaps: `no management key`, `no management team id`, `loading team prepaid...`, `team prepaid unavailable`.
- Team postpaid OAuth / Grok Build class and usage series when Management credentials work. That is not license message counts.
- A short double-entry spend section. Full view is `/spend`.

When two SuperGrok principals are stored, `/limits` stacks a section per principal. The live sampling line names which principal (or console key) is active when known. A second SuperGrok plan is visible only after a second `grok-oss login` that stores the Team principal. grok.com's account switcher is a different product.

Desired spend-order chrome (compact meter and `/limits` **Active:** line): spend included SuperGrok period limits on a stored personal SuperGrok login first. A Team / Business SuperGrok JWT is not the paying source while that personal login exists (that JWT settles as team postpaid OAuth / Grok Build and can debit the Billing Credits card). Then SuperGrok dollar credits that never expire, then console team prepaid / console API credits. Remaining included SuperGrok period limits across distinct stored plans are added together. That sum is the real remaining included quota. A unified pool (the same wire pool) counts once. While included SuperGrok period limits still have room, stay on SuperGrok session. After those included SuperGrok period limits are full, sampling hops to SuperGrok dollar credits, then to the console API as failover.

Only one `grok-oss` process fetches billing and limits. Other live TUIs read a snapshot under `$GROK_HOME`. There is no extra daemon. Rebuild SIGUSR1 is not this.

```
/limits
/limits --json
```

`/limits --json` prints the same machine-readable JSON as `grok-oss limits --json` into the conversation (no secrets). Fields include `schemaVersion`, `liveSampling`, and `activeDriver` (`supergrok_free_period` | `supergrok_extras` | `console_key`). Those `activeDriver` names are wire fields, not human meter names. `supergrok_free_period` is **included SuperGrok period limits**. `supergrok_extras` is **SuperGrok dollar credits** (prepaid SuperGrok top-ups). `console_key` is console team prepaid / console API credits. SuperGrok is paid. Never call SuperGrok free.

A grok-oss limits JSON or compact printout of included 100%, remaining 0, or SuperGrok dollar credits $0 must not mark SuperGrok used up or hop to console so this session cannot self-fix. grok-oss limits is a client printout, not xAI billing truth. Matching `nextReset` is not proof of a shared pool. Operator Usage (grok.com for that workspace) and the console.x.ai Billing page they can see win. Real SuperGrok HTTP 402 after that request failed can still leave SuperGrok. Never invent remaining. Never call any pool used up.

Named commands, same words on TUI `/limits` and CLI `grok-oss limits`: `stay-supergrok`, `use-console`, `meter included|dollar-credits|console|combined`, `refresh` (ForceRefresh). Pins live in `$GROK_HOME/limits_pins.json`, a sibling of `exhausted_credits/`. No new `[auth]` keys. Stock `preferred_method = "api_key"` still pins console. `stay-supergrok` hop-back does not require console credits. The compact meter names the driving meter (included SuperGrok period limits, SuperGrok dollar credits, console team prepaid / console API credits, or combined when remaining is across distinct SuperGrok identities). `/limits meter` chooses which of those named meters the compact line emphasizes.

See [Authentication](02-authentication.md#included-supergrok-period-limits-and-limits).

### `/privacy`

Open Settings on **Coding data, retention, and training**, where you choose
**Opt in** or **Opt out**. Takes no arguments.

```
/privacy
```

This setting doesn't touch `[features] telemetry`, `trace_upload`, or your external OTEL settings — see [Monitoring Usage](24-monitoring-usage.md#related-settings). On team accounts only a team admin can change it, and admins can also enable or disable Zero Data Retention for the team ([how to enable ZDR](https://docs.x.ai/developers/faq/security#how-to-enable-zdr)). When the choice isn't yours to make, the row says so — `ZDR` or `· Admin Managed` — instead of opening the chooser.

---

## Configuration and UI

### `/settings`

Open the settings modal to view and change configuration interactively. Aliases: `/config`, `/preferences`, `/prefs`.

### `/timestamps`

Toggle message timestamps on or off.

---

## Skills as Slash Commands

Any enabled skill with `user-invocable: true` in its SKILL.md frontmatter shows up as a slash command. (Turn a skill off via `/skills` and it stops being advertised.) So a skill at `~/.grok/skills/commit/SKILL.md` runs as:

```
/commit fix typo in README
```

Skills from plugins work the same way. When two skills share a name across scopes, qualify it:

```
/local:commit      # Project-scoped skill
/user:commit       # User-scoped skill
```

Built-in commands always win the bare name. Name a skill "compact" and `/compact` still runs the built-in — the skill stays available as `/local:compact` (or `/acme:compact` for a plugin). Both appear in the slash menu: the built-in is tagged `built-in` and the skill is tagged `skill · local` / `skill · acme`.

---

## Autocomplete

The menu supports fuzzy search: start typing after `/` to filter. Each entry shows the command name, its description, an argument hint when it takes arguments, and its source (builtin, skill scope, or plugin name). Press `Tab` or `Enter` to accept the highlighted command.
