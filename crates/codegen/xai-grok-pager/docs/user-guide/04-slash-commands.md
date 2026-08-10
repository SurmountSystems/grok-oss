# Slash Commands

Type `/` in the prompt to open the command menu. It fuzzy-matches as you type, and picking a command runs it immediately.

Commands come from two places: **shell builtins**, handled by the agent backend (xai-grok-shell), and **pager builtins**, handled by the TUI frontend (xai-grok-pager). Both show up in the same menu, and any enabled skill with `user-invocable: true` appears there too.

Every command below lists its aliases where it has them. A few commands only appear when a feature or session state enables them; those cases are called out inline.

---

## Session Management

### `/new`

Start a fresh session and clear the current conversation. Alias: `/clear`.

### `/resume`

Open the session picker to reload a previous session from disk.

### `/compact [context]`

Compress conversation history to reclaim context-window space. Pass a note to tell Grok what to keep:

```
/compact
/compact keep the auth implementation details
```

When the context window fills up, Grok auto-compacts at 95% usage by default
(configurable via `/settings` → **Auto-compact at**, or
`[session] auto_compact_threshold_percent` / `auto_compact_threshold_tokens` in
config.toml; UI choices 85 / 90 / 95 / 98% or Grok 4.5 card presets 200k / 475k tokens).
Percent thresholds apply to the *effective* window — with **Economic mode** on
(default), that window is soft-capped at 200k tokens (Grok 4.5 price cliff).

### `/economic-mode`

Cap (or uncap) effective context at 200k tokens for cheaper Grok 4.5 pricing.
Default **on** for new sessions (`[ui] economic_mode`). Soft-caps the context
window for compaction and the context bar.

Token Economy may rewrite **implement-loop effort** (skill reviewer fan-out
1–5, not model reasoning effort) on `/implement`: optional **lock** and
**min floor** always apply when set; economic mode + cap master still own the
hard ceiling (default **3**) and desired inject when missing (default **2**).
Toasts fire when the product rewrites effort. See
[Configuration → Token Economy](05-configuration.md#token-economy).

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

Generate a short "where was I" summary of the session so far. Alias: `/summarize`.
The summary is display-only (not added to the model conversation). Grok may also
request the same kind of recap automatically when you return after being away.

**Default on.** Search **Settings** (`/settings` / `/options`) for `recap` to
toggle auto return-from-away, the debounce threshold, and the master feature
kill. You can also set config or env:

| Goal | How |
|------|-----|
| Turn off **all** recaps (`/recap` + auto) | Settings → **Master session recap** off; or `[features] session_recap = false` / `GROK_SESSION_RECAP=0` |
| Turn off **auto** return-from-away only | Settings → **Auto session recap** off; or `[ui.notifications] session_recap = false` (manual `/recap` still works) |

Restart the session (or start a new one) after changing the master feature flag
so the shell re-advertises the gate. See [Configuration](05-configuration.md#session-recap).

### `/session-info`

Show session details — auth method, model, turn count, and context usage. Aliases: `/status`, `/info`.

### `/fork`

Branch the current session into a new agent, keeping history up to this point.

### `/rewind`

Roll the conversation back to an earlier turn and discard everything after it.

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

One-click **`⧉`** chrome uses the same stack: always-on per-bubble copy on user and assistant messages (`bubble_copy_buttons`, default on; no select-first), selection-box copy when a block is selected (`selection_buttons`, default on; selection-box omits ⧉ when bubble chrome is on so you never see two icons), plan panel top bar (whole plan body, same as **`Y`**), and the prompt top border (full draft plain text, including multimodal chip labels). `/copy` still targets the Nth assistant message only.

### `/export`

Export the conversation to a file or the clipboard.

### `/quit`

Quit the application. Alias: `/exit`.

### `/home`

Leave the current session and return to the welcome screen. Alias: `/welcome`.

### `/rename`

Rename the current session. Alias: `/title`.

```
/rename new session title
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

### `/always-approve` and `/auto`

Both are real toggles for the permission mode: they stay in the menu, and running the mode you're already in turns it back off.

| Command | When off | When already on |
|---|---|---|
| `/always-approve` | Skip tool permission prompts | Back to ask |
| `/auto` | Classifier approves safe tools (dangerous ones may still prompt) | Back to ask |

Running one while the other is active switches modes — for example, `/auto` while always-approve is on switches to auto. `/auto` only appears when the auto permission-mode feature is enabled. You can also change mode with `Shift+Tab` (cycles Normal / Plan / Always-approve), `Ctrl+O`, or `/settings`.

**Not plan approval:** `/always-approve` does not auto-approve a soft-parked plan. Plan decisions stay on the plan panel CTAs ([Plan mode](19-plan-mode.md#present-is-not-approval)).

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

Reopen the current session in the other render mode. `/minimal` (offered while you're in fullscreen) switches to the experimental scrollback-native mode; `/fullscreen` (offered while you're in minimal; alias `/full`) switches back to the standard alt-screen TUI. Both relaunch the pager on the same conversation for this session only — they don't touch `config.toml`, and the relaunch banner reminds you how to switch back. The `--minimal` / `--fullscreen` CLI flags are session-scoped the same way. To make plain `grok` open in a given mode by default, use `/settings` → **Default screen mode** or set `[ui] screen_mode`.

### `/plan`

Enter plan mode.

```
/plan [description]
```

### `/view-plan`

Open a preview of the current saved plan. Aliases: `/show-plan`, `/plan-view`.

### `/clear-completed-todos`

Remove **completed** and **cancelled** items from the live session todo board and archive them (toast reports how many). Pending and in-progress stay. Same as the todo pane **clear-finished icon** (`[−]` next to close when the todo board is open and finished rows exist, focused or not; quiet idle paint; does not cover tasks model/timer or subagent open chrome) and optional focused `X`. Action hints still say “Clear finished.” Not the same as pane `h` (hide done in the view only) and not an agent `merge: false` wipe.

```
/clear-completed-todos
```

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

The shell also advertises individual `/hooks-list`, `/hooks-trust`, `/hooks-add`, `/hooks-remove`, and `/hooks-untrust` commands; in the TUI pager these are folded into the `/hooks` modal.

### `/plugins`

Open the extensions modal on the Plugins tab to view installed plugins, install new ones from the marketplace, and manage trust.

The shell additionally supports subcommands (`/plugins list`, `/plugins install <source>`, `/plugins uninstall <name>`, `/plugins update`, `/plugins reload`). In the TUI, the modal does the same work visually.

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

Model-launched workflows may set `agent_budget` on the `workflow` tool. It's an absolute cumulative cap on logical child-agent calls: every `agent()` call and every item in a `parallel()` panel spends one slot, while schema-correction retries don't. The default is 128, explicit values run 1–1,024, and a panel that would cross the remaining budget is rejected before any of its children launch. `budget()` reports the cap as `total`, admitted calls as `spent`, `reserved` (always zero), and `remaining`. Named slash launches use the default budget.

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

Switch the TUI color theme. Alias: `/t`.

### `/feedback [message]`

Report an issue or send feedback.

```
/feedback Something isn't working correctly
```

### `/btw`

Send an aside to the agent without interrupting the current task. The side question and its answer aren't part of the main turn.

In the full TUI, a finished answer opens a **Done** panel:

- **`y`** (when the panel is focused) — copy the **full thread** to the clipboard (`/btw <question>` plus the complete rendered answer, not just what is on screen). The chrome also shows a `[y]` control.
- **`a`** — open a follow-up composer in the **same** btw session (prior Q/A is included for the model). You can keep asking without starting a new main turn.
- **`Esc`** — dismiss the panel.

In minimal mode (`--minimal`), the answer shows up in a dismissible panel above the prompt: `Esc` dismisses it, a finished answer is saved into native scrollback, and a late reply to an already-dismissed panel is dropped.

```
/btw also check the error handling
```

### `/note`

Leave a **mid-session operator note** that is **not** a pending main-turn prompt. Use this while a turn, plan approval, or background subagents are running when you want a personal annotation without enqueueing text that will hijack the agent when the parent becomes idle.

```
/note check queue hold when subagents finish
/note follow up on flake PATH #ci #hermetic
/note                  # list notes for this session
```

- Stores the note on the **current session only** (id, time, text, optional trailing `#tags`).
- Does **not** call the model, does **not** touch the prompt queue, and is not a substitute for short on-disk reports that agents write for other agents.
- Bare `/note` (or alias `/notes`) lists notes as a system block. `/tasks` also shows a count when notes exist.
- Full TUI confirms a save with a toast; minimal mode writes a short system line.

Promote-to-queue / promote-to-todo is intentionally deferred.

### `/mcps`

Open the MCP servers management modal.

### `/doctor`

Check the current session for terminal, clipboard, color, input, notification, and sandbox issues. Doctor shows what it found and how to resolve each issue. Run `/doctor fix` to list available automatic fixes; other findings include manual steps. `/terminal-setup`, `/terminal-check`, and `/terminal-info` remain aliases.

The dual-auth block also lists SuperGrok principal(s) (role + fingerprint only
when two logins are stored) and console key fingerprints. See
[Authentication → Two SuperGrok logins](02-authentication.md#two-supergrok-logins-personal--business).

### `/rebuild`

Rebuild this checkout's `grok-oss` binary and gracefully relaunch live instances
on this machine (same user and `GROK_HOME`). Distinct from worktree database
rebuild and from the SpaceXAI auto-updater channel.

1. Walks up from the process working directory to find a Grok OSS source tree
   (`justfile` + `crates/codegen/xai-grok-pager-bin`).
2. Runs `just install` (or fixed `cargo build` + install when `just` is missing)
   into `${CARGO_HOME:-~/.cargo}/bin/grok-oss`.
3. Verifies the installed binary's package version + git SHA.
4. Soft-signals reachable leaders to drain and exit for upgrade (same grace path
   as update relaunch; clients reconnect and reload the session).
5. Writes a cooperative relaunch request under `$GROK_HOME` and signals **every
   other live product TUI** registered in `active_sessions` (`SIGUSR1`) so those
   windows re-exec onto the new binary with the same session (not only the window
   that typed `/rebuild`). Mid-turn work uses canceled-turn-on-restart resume.
6. Re-execs **this** TUI onto the new binary with the same session id when
   possible. Mid-turn work is canceled with the normal canceled-turn-on-restart
   resume (re-queue once), not invent success.

CLI equivalent for agents and scripts: `grok-oss rebuild` (and optional
`--source <dir>`). After a successful rebuild, all active product windows on this
host should pick up the new binary; the rebuild report lists leaders and peer
TUI signal outcomes.

See also [Getting Started](01-getting-started.md) install notes and
`grok-oss update --check` (freshness vs Surmount `main` only; no auto-install).

### `/release-notes`

View release notes for the current version. Alias: `/changelog`.

### `/docs`

Browse the in-TUI How-to Guides, open the online Build docs, or jump straight to a guide by title. Aliases: `/howto`, `/guides`.

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

### `/personas`

Create, edit, and delete personas. A subagent can apply a persona to shape how it behaves.

---

## Account and Billing

### `/login`

Log in or re-authenticate without leaving the session.

### `/logout`

Log out and return to the login screen.

### `/usage`

View **session** token/cost totals, then SuperGrok billing when the consumer
surface is visible. Alias: `/cost`.

When free SuperGrok **billing period** bounds are known, also shows **linear-burn
pacing**: whether free SuperGrok period used % is ahead of or behind linear burn
for the period (never as dollars). When live sampling is a console key, SuperGrok
pacing is labeled as not the live principal. Full double-entry books are on
`/spend` and a section of `/limits`.

```
/usage
/usage manage
```

### `/spend`

**Token Economy double-entry:** local calculated spend (from session `usage.jsonl`
ingested into `$GROK_HOME/grok_oss.db`) side by side with remote Management
samples (team prepaid / postpaid / usage series when a management key is
available). Shows gap honesty when local cost ticks are missing so USD gap is
not comparable. Meters stay distinct (free SuperGrok period % ≠ SuperGrok top-up
$ ≠ console team prepaid ≠ API vs OAuth class). Aliases: `/double-entry`,
`/ledger`.

```
/spend
```

### `/limits`

Opens a **dismissible popup** (Esc to close) with spend meters from cached
billing (not session tokens). Does not dump a static block into the chat.
While the popup is open, **Resets in: Xd Yh Zm Ws** ticks live; when the
countdown hits zero, billing re-fetches so meters continue after period reset.
Keeps each meter distinct:

- SuperGrok **included** weekly/monthly allowance (used % · remaining % · next reset · live countdown)
- Free SuperGrok period **linear-burn pacing** when period bounds exist
  (ahead/behind linear burn; omit when bounds missing; console-live honesty)
- SuperGrok **dollar extras** (prepaid session balance; separate from included)
- **Console API key** — `Requests: console` when the console chat key is
  handling requests, `Requests: SuperGrok` when that key is on file but
  SuperGrok is handling requests, or `no key` when no console chat/API key
  exists. Key presence is implicit when a request path is shown; a key on file
  never looks missing just because SuperGrok is live.
- **Console team prepaid** (Management API balance when a **management** key and
  `management_team_id` / `XAI_MANAGEMENT_TEAM_ID` are set, or team id is
  discovered from the management key). Shown under **Console API** even when
  SuperGrok is live (`console.isLive` false). Otherwise distinct honest gaps:
  `no management key`, `no management team id`, `loading team prepaid...`, or
  `team prepaid unavailable`. Store the key with `grok login --management-key`
  (or `XAI_MANAGEMENT_API_KEY`). The management key is **not** the same as the
  console inference API key and is **not** part of the Key line above. This is
  **not** SuperGrok extras and **not** a Business SuperGrok OIDC login.
  Setup: [Authentication → Console team prepaid](02-authentication.md#console-team-prepaid-management-api).
- **Team postpaid** (OAuth vs API class) and **usage series** day window when
  Management credentials work. Dollar class only, not license message counts.
  Series refreshes on the same path as prepaid/postpaid: TUI `/limits` open,
  background billing refresh (about every 60s soft cache), and `grok limits`.
  When OAuth / **Grok Build class** is known and positive, that line is near the
  top of the Console block (dogfood settlement proof), still separate from team
  prepaid **Balance**.
- Honesty notes include: SuperGrok included % is a poll reading (not proven burn);
  the console **Platforms → Grok Business licenses** page (messages /
  conversations) is **not dogfood proof** (zeros expected for CLI SuperGrok).
  Real burn is **team Usage** dollars (browser team Usage / spend / Grok Build)
  and Management postpaid / series plus SuperGrok meters.
- A short **double-entry spend** section (local book vs remote); full view is
  `/spend`.

When **two SuperGrok principals** are stored (personal + Business), `/limits`
stacks a section per principal (for example `SuperGrok (personal)` and
`SuperGrok (business)`). The live sampling line names which principal (or
console key) is active when known. The non-active sibling may show **no data
yet** until its billing pool has been polled. Personal included, Business
included, SuperGrok dollar extras, and console team prepaid stay separate lines.

The **status bar** (top row, right side next to context tokens) always shows a
compact meter that matches the **live sampling principal** in Build sessions —
including team dual-auth SuperGrok and console-live sampling, not only personal
consumer billing. That compact meter is **spend-order chrome** (the same order
as `/limits` **Active:** / `activeDriver`), not proof of which team wallet
settles the bill. Meters stay distinct:

- **SuperGrok live:** `free SuperGrok period · N%` when free SuperGrok period
  usage is known (true `free SuperGrok period · 0%` is allowed), optionally with
  a short linear-burn chip when period bounds exist. While billing has not
  returned a real free SuperGrok period reading, chrome shows
  `free SuperGrok period · ...%` (not a silent `0%` lie). When free SuperGrok
  period is full and SuperGrok dollar credits remain, the compact meter switches
  to `SuperGrok extras · $N`. Background poll keeps refreshing until the free
  SuperGrok period meter is known, near exhaust, or when OpenRouter / console
  prepaid is active.
- **Console key live:** `console · $N` (team prepaid) or the honest gap strings
  (`no management key`, `no management team id`, `loading team prepaid...`,
  `team prepaid unavailable`). Never free SuperGrok period % as the live spend.

Gateway/chat sessions hide the coding-credits meter. Click that meter to open
this same `/limits` popup. The prompt footer is a one-line summary for SuperGrok
warnings and/or **secondary team wallet** context when the consumer billing
surface is on (personal SuperGrok principal; not team AuthMeta enterprise hide,
not API-key primary alone):

- **Console live:** `Console key · team prepaid: $N` (or honest gap strings);
  optional `team Grok Build class: $N` when postpaid OAuth class is in cache.
  Under console live, team prepaid is the live console pool.
- **SuperGrok live while free SuperGrok period has room:** footer shows only
  SuperGrok free-period / SuperGrok dollar credits warnings when those fire.
  Team prepaid remaining and Grok Build class stay off the prompt footer so they
  do not dominate next to model name (full team wallets stay on `/limits`).
  Compact status still names free SuperGrok period.
- **SuperGrok live after free SuperGrok period is full:** free SuperGrok period
  remaining / SuperGrok dollar credits when those warn, plus optional secondary
  team line such as
  `not the active spend path: team prepaid remaining $N · Grok Build class $M`
  (or cold management `not the active spend path: loading team prepaid...`).
  That secondary line never means "you are paying team prepaid now" and never
  re-labels live sampling as console. Missing management key still keeps
  SuperGrok-only footers SuperGrok-focused (team honesty stays on `/limits`
  Balance).

Billing refresh (session start, turn end, `/usage`, force-refresh on `/limits`)
fills SuperGrok cache and, when configured, Management team prepaid **and**
postpaid into process cache regardless of which principal is live.

```
/limits
/limits --json
```

**`/limits --json`** skips the popup and prints the same machine-readable
JSON as CLI `grok limits --json` into the **conversation transcript** (pretty
JSON in a fenced code block). Both you and the agent can see it in session
history. Fields include `schemaVersion`, `liveSampling`, **`activeDriver`**
(`supergrok_free_period` | `supergrok_extras` | `console_key`), SuperGrok
principal meters, and console team prepaid (no secrets).

Human `/limits` and `grok limits` lead with **Live sampling** then **Active:**
(free SuperGrok period | SuperGrok extras | console key), matching status
compact chrome under Design A. Free SuperGrok period always comes before
SuperGrok dollar extras and console credits while free period has headroom.

Outside the TUI, agents and scripts can query the same meters (live sampling
principal + SuperGrok included % / dollar extras + console team prepaid when
configured) without pasting a screenshot:

```bash
grok limits           # human multi-line report (includes Active: driver)
grok limits --json    # machine-readable JSON (schemaVersion 1; no secrets)
grok limits multipoll # N samples + P1 path / P2 free SuperGrok period series
just limits-multipoll # same harness via just (see Authentication checklist)
```

**`grok limits multipoll`** takes several live `limits --json` samples (default
2, default **30s** sleep between sample ends so the flat-poll detector can
see a ≥30s wall window). It writes `samples.jsonl`, `fields.jsonl`, and
`summary.json` under `.agents/reports/limits-multipoll-<utc>/` (or
`--out-dir`). Plain summary on stdout names **P1 path** (OK or fail) and
**P2 free SuperGrok period limits** (flat / stepped / insufficient). Exit is
**0** when the path is OK; exit is **non-zero only on path failure** (for
example console live while free SuperGrok period limits still have room).
Free SuperGrok period staying flat (for example 6%) is **measurement only**
and does **not** fail the process by itself. That is not proof the client left
free SuperGrok period limits; it is ticket evidence that free-period debit is
still unproven on the server.

Desired spend order while free SuperGrok period limits still have room: stay
on SuperGrok session (free SuperGrok period limits first, then SuperGrok
dollar credits, then console team prepaid / console API credits). Never invent
free SuperGrok period used % on the client.

Full operator checklist: [Authentication → Token economy proof checklist](02-authentication.md#token-economy-proof-checklist).

### `/privacy`

Show or toggle privacy and data-retention status.

```
/privacy
/privacy opt-in
/privacy opt-out
```

`/privacy` doesn't touch `[features] telemetry`, `trace_upload`, or your external OTEL settings — see [Monitoring Usage](24-monitoring-usage.md#related-settings). On team accounts, only a team admin can toggle privacy this way, and admins can also enable or disable Zero Data Retention for the team ([how to enable ZDR](https://docs.x.ai/developers/faq/security#how-to-enable-zdr)).

---

## Configuration and UI

### `/settings`

Open the settings modal to view and change configuration interactively. Aliases: `/config`, `/preferences`, `/prefs`, `/options`.

### `/timestamps`

Toggle message timestamps on or off.

### `/screenshot`

Capture the **current rendered TUI frame** as a PNG (not an OS screenshot of
other apps). Same action is bound to **F9**. Writes under
`$GROK_HOME/screenshots/tui-*.png` (default `~/.grok/screenshots/…`) and toasts
the path. When plan approval is open, the capture also auto-attaches into the
plan multimodal path; you can still open the toast path and paste manually.

```
/screenshot
```

Window title vs in-app chrome: **`[ui.notifications.title] enabled`** (default
true) controls dynamic terminal/tab **window titles**; **`[ui] hide_header`**
hides in-app status / welcome / dashboard headers. They are separate. See
[Theming → Hide header](06-theming.md#hide-header) and
[Window title](06-theming.md#window-title).

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

Built-in commands always win over a skill with the same name. Name a skill "compact" and `/compact` still runs the built-in — but `/local:compact` invokes the skill.

---

## Autocomplete

The menu supports fuzzy search: start typing after `/` to filter. Each entry shows the command name, its description, an argument hint when it takes arguments, and its source (builtin, skill scope, or plugin name). Press `Tab` or `Enter` to accept the highlighted command.
