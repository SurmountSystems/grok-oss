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

### `/dashboard`

Open the [Agent Dashboard](23-dashboard.md): live roster of top-level sessions in this pager (peek, reply, dispatch, pin, rename, stop, attach). Aliases: `/agents-dashboard`, `/sessions`.

Not `/config-agents` (alias `/agents`), which manages agent *definitions* and personas. Hidden in minimal mode; disable with `GROK_AGENT_DASHBOARD=0` or `[dashboard].enabled = false`.

### `/compact`


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

Grok also auto-compacts once the context window hits 85% (tune it with `[session] auto_compact_threshold_percent`).

### `/context`

Show the context window split into System prompt, Messages, Reasoning/overhead, and Free.
Rows for Tool definitions, Skills, and MCP servers are already counted in those totals.

### `/recap`

Generate a short "where was I" summary of the session so far. Alias: `/summarize`. The summary is display-only (not added to the model conversation). Grok OSS may also request the same kind of recap automatically when you return after being away.

**Default on.** Search `/settings` for `recap` to toggle auto return-from-away (`[ui.notifications] session_recap`), the debounce (`session_recap_threshold_secs`, default 30), and the master feature (`[features] session_recap`, restart required). `GROK_SESSION_RECAP=0` kills both `/recap` and auto recap.

### `/session-info`

Show session details — auth method, model, turn count, and context usage. Aliases: `/status`, `/info`. Click a value or drag to select and copy; `c` copies the session ID and `y` copies the whole block.

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

Restate this session when you cannot parse the last agent chat. Not an apology. The agent replies with four labeled complete thoughts only: **Job**, **State**, **Operator** (or `nothing`), **Next**. Speaker labels are Operator not You or Human, and Agent not Me or Grok when Grok means the assistant. Optional focus text is passed through. Follow Concise American Technical English as specified in Surmount `0005_CATE.md`.

This is **not** `/recap` (a short chat recap), **not** `/finish` (session post-mortem), and **not** `/reports` (a checkpoint file). Complete American English thoughts. No leftover board ids as the body.

`/what` is a **default Grok OSS skill** (in-tree `crates/codegen/xai-grok-bundle/skills/what`, installed into `~/.grok/bundled/skills/what`; not host overlay as the grok-oss source, not a pager-only prompt, not a project `.agents/skills/what` pack). The pager builtin `/what` keeps that slash name; a same-named skill cannot steal the bare command. Immediate `/what` injects that skill now. When you ask to revise a skill in grok-oss, edit `crates/codegen/xai-grok-bundle/skills/`. The live cache is not the source.

```
/what
/what the last status
```

### `/pull-remote-tree`

Pull a remote project tree onto a local directory. This is a **default Grok OSS skill**. New grok-oss users get it without adding a project pack. Grok installs it from the product tree (`crates/codegen/xai-grok-bundle/skills/pull-remote-tree/`) into `~/.grok/bundled/skills/pull-remote-tree/` on startup. It is not a pager builtin and not a host overlay skill. It is not a project skill at `.agents/skills/pull-remote-tree/`.

The named tool is `pull_remote_tree`. Direction is `HOST:SRC` (or a local source directory) onto a local dest only. Dest that looks like `HOST:PATH` is refused. Copy is a Rust walk plus `std::fs`. OpenSSH may fetch a remote tree. This is not a rsync tool id. Never git commit.

```
/pull-remote-tree
/pull-remote-tree host:/var/src /tmp/dest
```

### `/metadata`

Show live session context: grok-oss ULID, Grok Build / ACP UUID, working directory, model, when this window started, and this process id. Fields that are not known are omitted rather than invented. `/settings` **ULID session ids** (default on) chooses which id is listed first. The map still exists when that toggle is off. Not `/session-info` (auth, turn count, and context usage).

### `/fork`

Branch the current session into a new agent, keeping history up to this point.

### `/rewind` (alias: `/undo`)

Roll the conversation back to an earlier turn and discard everything after it. `/undo` is the same command.

To edit an **existing** draft when a terminal or multiplexer reserves `Ctrl+G`, open the command palette and select **Edit Prompt in External Editor**. That direct route preserves the existing text and refuses pasted, file-reference, or image chips without flattening them. Typing `/edit-prompt` into the composer necessarily replaces that input, so it starts from an empty draft.

### `/copy`

Copy the most recent response's source markdown to the clipboard. Pass a number to copy the Nth-latest response instead, or a file path to write the text to a file rather than the clipboard (handy over SSH, where the local clipboard is often unreachable).

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

---

## Model and Mode

### `/model <name>`

Switch models. Accepts a model ID or display name (case-insensitive). When the model offers more than one context window, you can add a window size next. For reasoning models you can add an effort level last. The picker asks in the same order: model, then window, then effort. Alias: `/m`.

```
/model grok-4.6
/model Grok 4.6
/model Reasoning X high
/model Reasoning X 500k high
```

### `/effort <level>`

Set reasoning effort on the **current** model without reselecting it. Levels are `low`, `medium`, `high`, and `xhigh`, and it only applies when the active model supports reasoning effort.

```
/effort high
```

### `/context-window <size>`

Set the context window for the current model. The command is listed only when the model supports more than one size. It accepts the short label or the raw token count. The choice lasts for the session. It carries over to a new model that supports the same size. A size smaller than the current usage starts an auto-compact.

```
/context-window 500k
/context-window 256000
```

### `/always-approve` and `/auto`

These are real toggles for the permission mode: they stay in the menu, and running the mode you're already in turns it back off.

| Command | When off | When already on |
|---|---|---|
| `/always-approve` | Skip all permission prompts | Back to ask |
| `/auto` | Auto-review: a classifier approves safe tools (dangerous ones may still prompt) | Back to ask |

Running one while the other is active switches modes — for example, `/auto` while always-approve is on switches to Auto-review. `/auto` only appears when the Auto-review permission-mode feature is enabled. You can also change mode with `Shift+Tab` (cycles Normal / Plan / Auto-review (when enabled) / Always-approve), `Ctrl+O`, or `/settings`.

### `/multiline`

Toggle multiline input. When it's on, `Enter` inserts a newline and `Shift+Enter` (or `Alt+Enter`) sends the message. Mid-turn, a bare `Enter` on an empty composer still force-sends the top queued follow-up. Alias: `/ml`. With multiline off, the composer footer shows the newline chord once the draft is non-empty.

### `/history`

Open prompt-history search: fuzzy-search this session's prompts newest-first, then press `Enter` or `Tab` to drop a match back into the prompt.

For quick recall, press `↑` on an empty prompt instead. With prompts queued, that moves focus into the queue pane, highlighting the last row; otherwise the panel opens with your most recent prompt already filled in, and `↑`/`↓` step through entries (each lands in the input), `↓` past the newest entry closes the panel, and typing edits the recalled prompt in place.

### `/compact-mode`

Toggle compact display — less padding and tighter spacing for denser output.

### `/vim-mode`

Toggle vim-style scrollback keys (`j`/`k`, `h`/`l`, `g`/`G`, `y`/`Y`, and so on). With it off (the default), a bare letter or `Shift+letter` in the scrollback just focuses the prompt and types the character. The setting persists to `[ui] vim_mode`.

### `/edit-prompt`

Open an external editor for the prompt, in either render mode. Grok resolves `$VISUAL`, then `$EDITOR`, then `vi`; command values may include quoted arguments. Saving replaces the draft without sending it, and saving an empty file clears it. Typing `/edit-prompt` necessarily replaces the composer's contents, so the editor starts from an empty draft; to edit an **existing** draft, choose **Edit Prompt in External Editor** from the command palette (or press `Ctrl+G` in minimal mode), which preserves the text and refuses pasted, file-reference, or image chips without flattening them.

```
/edit-prompt
```

### `/minimal` and `/fullscreen`

Switch the current session to the other render mode, in place. `/minimal` (offered while you're in fullscreen) switches to the experimental scrollback-native mode; `/fullscreen` (offered while you're in minimal; alias `/full`) switches back to standard fullscreen mode. The switch happens inside the running process — nothing restarts, so a running turn keeps streaming and your composer draft, queued prompts, and permission mode all carry over; a marker (committed line in minimal, toast in fullscreen) reminds you how to switch back. Both are session-scoped — they don't touch `config.toml` — and the `--minimal` / `--fullscreen` CLI flags are session-scoped the same way. To make plain `grok` open in a given mode by default, use `/settings` → **Default screen mode** or set `[ui] screen_mode`. (If the in-place transition misbehaves in an exotic terminal, `GROK_SCREEN_MODE_SWITCH=exec` restores the old behavior of relaunching the pager onto the same session.)

A handful of commands only work in one of the two modes, because the surface they drive doesn't exist in the other: `/find`, `/jump`, `/timeline`, `/theme`, `/tutorial`, and `/dashboard` are fullscreen-only, while `/expand` is minimal-only. (`/workflow runs` is different: it opens the run pane in fullscreen and degrades to a text overview in minimal rather than refusing.) Those are hidden from the command menu and the palette in the mode they can't run in. If you type one out anyway, Grok says why — and points you at whichever is actually useful. When the other mode is the only way to get it, that's the mode switch: `/theme isn't available in minimal mode (minimal renders with your terminal's own palette). Run /fullscreen to switch this session.` When this mode already does the job another way, it names that instead: `/expand isn't available in fullscreen mode: press Tab to focus the scrollback, then → on the block.` Everything else works in both. Note that `--no-alt-screen` still counts as fullscreen here, so it keeps the fullscreen-only commands.

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

`/flush` and `/dream` require memory enabled through `GROK_MEMORY=1`, `[memory] enabled = true`, or managed remote settings. `/memory` is available whenever a memory store is configured for the session, including when `[memory] enabled = false` turned memory off, so you can browse saved notes and turn memory on for the session from inside the modal. It is hidden only when no memory store is configured, or when `--no-memory` / `GROK_MEMORY=0` turned memory off for the whole process. `/remember` is always available.

### `/memory`

Browse, view, and manage saved memories. Alias: `/mem`. Inside the modal, `t`
turns memory on or off for the session, `x` deletes the selected note, and `s`
shows content-free queue, lease, retention, and pinned-rollout diagnostics.
The `t` toggle is session-scoped: it does not edit `config.toml`, and new
sessions follow the config again. It cannot override `--no-memory` or
`GROK_MEMORY=0`.

```
/memory
```

### `/flush`

Save the current session's knowledge to memory right now, triggering an LLM summary of the most important content. Reach for it before compaction, or any time you want to lock in context. The status line shows "Flushing memory…" while it runs, and a scrollback line reports the outcome (for example "Memory flushed through turn 12").

### `/dream`

Run memory consolidation — merge session logs into organized topics. The status line shows "Consolidating memory…" while it runs, and a scrollback line reports what happened: how many observations were merged into how many topics, that there was nothing to consolidate, or that another session is already consolidating.

### `/remember`

Save a note to memory immediately, without waiting for an automatic summary.

```
/remember the staging deploy uses the eu-west cluster
```

---

## Hooks and Plugins

`/hooks`, `/plugins`, `/marketplace`, `/skills`, and `/workflows` all open the same extensions modal, each on its own tab.

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

The command returns right away — follow progress in `/workflow runs`, and the final report appears in the conversation on its own.

Workflows use an absolute cumulative `agent_budget` cap on logical child-agent calls: every `agent()` call and every item in a `parallel()` panel spends one slot, while schema-correction retries don't. The default is 128, explicit values run 1–1,024, and a panel that would cross the remaining budget is rejected before any of its children launch. Model-launched workflows set `agent_budget` on the `workflow` tool; named slash launches accept `--agent-budget N` or an `agent_budget` field in their JSON args. Named launches can also set child reasoning effort with `--effort LEVEL` or JSON `effort`, without changing the current session's `/effort`; a child script's own `effort` option takes precedence. Separately, a host-configured cap (32 by default) bounds how many children run at a time per run; larger panels queue and still act as a barrier. `budget()` reports the cap as `total`, admitted calls as `spent`, `reserved` (always zero), and `remaining`.

### `/workflow`

Launch a saved workflow, or manage a running one by its session-unique display name. Launch the same workflow twice and the display names are numbered (`review-changes`, `review-changes-2`); you never need the internal run IDs. Bare `/workflow` prints a text overview of this session's runs.

Type `/workflow` and a space to autocomplete saved workflow names (built-in, project, and user) plus the manage verbs `runs`, `pause`, `resume`, `stop`, and `save`. Picking a name fills it in and offers launch flags before you add args; it does not launch until you press Enter. `pause` / `resume` / `stop` / `save` then list this session's run handles — a bare `/workflow stop` does not pick a run.

```
/workflow review-changes --agent-budget 256 --effort high {"target":"origin/main...HEAD"}
/workflow review-changes {"target":"origin/main...HEAD","agent_budget":256,"effort":"high"}
/workflow runs
/workflow pause review-changes
/workflow resume review-changes
/workflow stop review-changes-2
/workflow save review-changes
```

`/workflow runs` opens the live **Workflow Runs** dashboard in the fullscreen TUI — active and retained runs, not a catalog of saved definitions. Each row shows the run's display name, phase, agent roster, progress, and result. Inside a run's detail view, `p` pauses, `r` resumes an ordinary pause, and `x` stops. Budget-limited runs can't bare-resume: `r` returns the shell's rejection (raise the cap with a model/tool resume that passes a higher `agent_budget`), while `x` still stops. `s` saves the run's script, but it's hidden for known built-ins and numbered duplicate handles — for those, choose a new unique `meta.name` and save the edited script explicitly. In minimal mode and non-TUI clients, `/workflow runs` prints the same text overview as bare `/workflow`.

Project workflows live in `.grok/workflows/*.rhai`; user workflows live in `~/.grok/workflows/*.rhai`. A same-process pause/resume continues the original immutable script, args, and `agent_budget` cap from committed host-call results — to iterate, edit the returned script copy and launch it as a new run.

A budget-limited run is different: it only resumes through a model/tool resume request that supplies an `agent_budget` above the admitted agent count. A bare `/workflow resume <name>` can't raise the cap, so it rejects budget-limited runs. Runs interrupted by a process restart aren't resumed at all, because external effects have no stable cross-process identity. And resume is not exactly-once: an external effect whose result wasn't committed before a same-process pause can run again.

### `/workflows`

Open the extensions modal on the **Workflows** tab — a browse-only catalog of the saved workflows Grok discovered (built-ins, project `.grok/workflows/`, and user `~/.grok/workflows/`), with each entry's source, description, and path. The same catalog is listed for the model under the skill listing in the session preamble. Launch one with `/workflow <name>` (or its own slash command), then watch it in `/workflow runs`.

---

## Other

### `/theme`

Switch the color theme. Alias: `/t`.

### `/feedback [message]`

Report an issue or send feedback. Bare `/feedback` opens the feedback form in every mode, including `--minimal`. Its **Write** tab is a report box: `Enter` sends, `Esc` closes. Its **Drafts** tab (`Ctrl+Tab` switches) holds reports saved for later — failed sends and feedback the agent drafted for you — and `Enter` loads one into Write so you can review, pick a type, and send it. `/feedback <message>` sends the message immediately, in any mode; if the send fails, the message is saved to Drafts.

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

`/btw` can also appear mid-message: the whole message (minus the token) becomes the side question and nothing goes to the main turn. Only `/btw` works this way; other commands must start the message. For a multi-line side question, use `Alt+Enter` (over SSH) or `Shift+Enter`, a trailing `\`, or `/ml`. Do not rely on `Cmd+Enter`: Apple Terminal inserts a newline locally via CoreGraphics; a delivered `SUPER+Enter` (Kitty) also inserts a newline rather than sending; over SSH Cmd never arrives and the chord sends.

```
/btw also check the error handling
fix the retry loop first. /btw what does WBC stand for?
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
6. Re-execs this TUI. Mid-turn work uses continue interrupted turn (`canceled_turn_resume.json`), not invent success. An unsent composer draft, queued prompts (including mid-turn interject text), plan Human-box notes, and session `plan.md` survive that relaunch the same way they survive a disconnect. This TUI persist path does not cancel nested subagent ids, and `/rebuild` is not blocked until nested work finishes. Ctrl-C quits and does not re-exec peers. Operator Enter send, mid-turn interject, queued prompts, and plan Human-box notes that ride Approve are also appended to the session-local write-ahead log (`prompt_wal.jsonl`) before the model is asked and before this re-exec. That file is how a dropped prompt can be restored as a pending Human turn. `/rebuild` persist writes the same record format (`rebuild-flush`).

Nested work on the leader survives `/rebuild` the same way it survives a TUI disconnect: the leader process stays up while nested ids are live, and those ids are not cancelled. After nested ids finish, this leader process stays up while the parent turn is still busy, the same way a dropped TUI leaves the leader up until that turn is idle. There is no five-second parent-turn cap. Then the leader may relaunch onto the new binary. Named tests: `relaunch_drain_keeps_nested_ids_alive_after_grace_like_disconnect`, `relaunch_drain_keeps_parent_turn_until_idle_like_disconnect`.

To roll back after a successful install, copy `${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss.prev` over `${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss` and make that file executable. That sibling file is the previous grok-oss binary from the last `/rebuild` that found an existing install.

CLI: `grok-oss rebuild` (optional `--source DIR`) compiles and signals live grok-oss instances the same way `/rebuild` does, without re-execing this process (there is no TUI here). Freshness only: `grok-oss update --check` (compare to Surmount `main`; no auto-install).

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

Inside a session this opens the usage modal with the account allowance plus that session's context and token totals. From the [Agent Dashboard](23-dashboard.md#dispatch-input) the same modal opens over the dashboard; there is no session there, so only the **Usage limit** tab carries data.

For persisted per-turn token and cost totals of any local session, use `grok usage <session-id> [turn]` from the shell. See [Session Management](17-sessions.md#the-grok-usage-subcommand).

### `/privacy`

Open Settings on **Coding data, retention, and training**, where you choose
**Opt in** or **Opt out**. Takes no arguments.

```
/privacy
```

This setting doesn't touch `[features] telemetry`, `trace_upload`, or your external OTEL settings — see [Monitoring Usage](24-monitoring-usage.md#related-settings). On team accounts only a team admin can change it, and admins can also enable or disable Zero Data Retention for the team ([how to enable ZDR](https://docs.x.ai/developers/faq/security#how-to-enable-zdr)). When the choice isn't yours to make, the row says so — `ZDR` or `· Admin Managed` — instead of opening the chooser. ZDR locks coding-data sharing; it does not mute external OTEL or `user.email` — see [ZDR and this stream](24-monitoring-usage.md#zdr-and-this-stream).

---

## Configuration and UI

### `/settings`

Open the settings modal to view and change configuration interactively. Aliases: `/config`, `/preferences`, `/prefs`.

### `/timestamps`

Toggle message timestamps on or off. When on, each message keeps its clock on the first visible row even if the original timestamp line has scrolled above the fold.

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
