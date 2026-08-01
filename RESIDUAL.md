# Open residual (human intent and unfinished honesty)

**D0 — open only.** Finished work lives in [`FORK.md`](FORK.md), process docs,
or code — not only here. Closed campaign history:
[`doc/dev/campaigns/interject-todos-closed-2026-07.md`](doc/dev/campaigns/interject-todos-closed-2026-07.md).

## Open

- **Plan approval UI (product chrome shipped → FORK; agent freeform still soft):**
  soft-park **auto-opens** the plan side panel; footer mouse CTAs hit-tested;
  card re-reads live `plan.md`; L1 typing stays modal-free. `/view-plan` still
  reopens if dismissed. Lasting truth: [`FORK.md`](FORK.md). Joins under
  `/tmp/grok-join-impl-*plan*`. **Still soft:** agent-written `plan.md` can
  invent freeform "reply approve / options 1–5" (product chrome does not;
  process law = product CTAs only). Process: `bug:exit-plan-mode-false-approve`.

- **Stuck Retrying / stream headers (shipped → FORK; soft dogfood only):**
  product truth in [`FORK.md`](FORK.md) (StreamResumed clear, 120s headers
  timeout, cancel-aware cooldown, short transport labels). Soft not shipped:
  phase-timer "since retry"; remaining-seconds on non-429 shared-wait.
  **Dogfood** after `just install` if the stable binary lags the tree. Join:
  `/tmp/grok-join-impl-stuck-retry-fix-2026-07-30.md`.

0. **Structured todos: fib leaves + progress + no casual reset (shipped product)**
   **Shipped:** first-class optional `size` (1|2 only; reject 0/3/5/8…);
   `meta.size` fallback normalized into field; leaf-only
   `compute_leaf_progress` (points mode when any leaf sized; else legacy
   counts); reject size on parents with children; tool result includes
   `progress` + optional `merge:false` archive warning; status-bar badge
   shows `N/M pts` in points mode; `prompt.md` Planning + tool description
   teach merge-only + fib leaves. **Clear finished** (open board + finished
   `[−]` chrome; quiet idle; non-overlap with tasks subagent chrome /
   optional focused `X` / `/clear-completed-todos` + `SHELL_RESERVED`),
   **click tasks model/timer/`[↗]` → open subagent**, **Worked-for one live
   line**, **soft-park three surfaces**, caret Human green + no residue, and
   lower-left throbber magenta → lasting truth in [`FORK.md`](FORK.md); not
   open residual. **Parked:** inventing colour for an unnamed “little guy”
   glyph until the operator names the control.
   **Still soft:** no hard ban on inventing bare ids; phase vs work tree is
   agent structure (not enforced hierarchy product); no archive browser UI.
   Plan:
   [`.agents/plans/plan-todo-progress-fib.md`](.agents/plans/plan-todo-progress-fib.md).
   Brief: [`doc/dev/research/todo-progress-fib-2026-07-26.md`](doc/dev/research/todo-progress-fib-2026-07-26.md).

1. **UDAX TOON (T0–T6 shipped; closed)**
   **Shipped:** `util/toon` + Dynamic tool-result path + env policy + **MCP**
   densify-before-`mcp_truncate` + first-class **`json_to_toon`** tool +
   **T5** model-facing densify for subagent handoff / task output / Text pure
   JSON / SearchTool / SchedulerList / child task prompt (shared
   `densify_structured_text`; free text + protocol envelopes unchanged;
   on-disk `prompt_context.json` stays JSON) + **T6** fail-open debug
   savings line (`before_bytes` → `after_bytes`). Soft remainders only if
   dogfood finds a new model-facing JSON chokepoint. Detail:
   [`doc/dev/research/udax-json-toon-2026-07-26.md`](doc/dev/research/udax-json-toon-2026-07-26.md).

2. **Plan approval soft park (side panel auto-open shipped → FORK; freeform soft)**
   **Shipped:** `exit_plan_mode` soft path parks durable approval, keeps draft,
   **auto-opens** non-capturing plan **side panel** (toast/status name side
   panel, not a `/view-plan` nudge). L1 stays modal-free (printable → composer).
   Approve/quit via **mouse footer CTAs**, side panel, status chip; `/view-plan`
   reopens if dismissed. Force fullscreen: `plan_approval_park = "modal"`.
   Lasting truth: [`FORK.md`](FORK.md). Design note (historical):
   [`doc/dev/research/plan-modal-softer-park-2026-07-26.md`](doc/dev/research/plan-modal-softer-park-2026-07-26.md).
   Joins: `/tmp/grok-join-impl-plan-paste-and-auto-open.md`,
   `/tmp/grok-join-impl-l1-modal-free-plan-2026-07-29.md`.
   **Still soft:** agent-written `plan.md` freeform menus; toast may still
   *feel* modal to some operators. Do not invent a third park mode.

2d. **Plan approval: real clickable CTAs + fresh plan.md (product chrome shipped
   2026-07-29; agent plan.md freeform still open)**
   **Shipped:** soft-park footer mouse CTAs (hit-tested; draft durable; Prompt
   focus paint); scrollback card not a fake button menu; empty placeholder not
   a key list; FileBacked panel + soft-park card re-read live `plan.md` (in-place
   card update, no second-card spam). Boards `bug:plan-cta-no-click-buttons`,
   `bug:plan-approval-stale-snapshot` product side green. Joins under `/tmp/grok-join-impl-*plan*2026-07-29.md`.
   **Still open:** agent-written `plan.md` body can still invent freeform chat
   menus; product does not inject that chrome. Do not claim agent ceremony gone.

2e. **TUI self-screenshot (v1 + F9 + plan auto-attach shipped 2026-07-29; font soft)**
   **Contract (plain language):** capture the **current rendered TUI frame** as
   a PNG for dogfood, agent attach, and plan comments — not an OS screenshot of
   other apps, not a second share-sheet. Board: `feat:tui-self-screenshot`.
   **Shipped:** `/screenshot` slash → last presented ratatui buffer → PNG under
   `$GROK_HOME/screenshots/tui-*.png` + toast path; pure encode helpers in
   `xai-grok-pager-render::tui_screenshot` (TDD); **F9** global chord; when plan
   approval is open, capture auto-attaches the PNG into the plan multimodal
   path. Joins: `/tmp/grok-join-impl-tui-self-screenshot-2026-07-29.md` (v1);
   `/tmp/grok-join-impl-screenshot-soft-2026-07-29.md` (F9 + plan auto-attach).
   Tests: `capture_tui_screenshot_bound_to_f9_always`,
   `try_attach_tui_screenshot_for_plan_when_approval_open`.
   **Still soft:** richer font raster (v1 uses simple ink marks). Operator can
   still open the toast path and paste.

2f. **Sapient Experience (SX) + plain English + thoughtful names (OPEN)**
   Operator intent (2026-07-27): **Sapient Experience** is how tools talk with
   humans. Source PDF: `/home/hunter/Documents/Sapient Experience.pdf`. Host
   law: `~/.grok/AGENTS.md` § Sapient Experience (under Prose + tone).

   Stance (from PDF + operator clarification same day):
   - SX names usable function, not soul/ontology. Stay a tool.
   - **Speak to humans as humans do; do not try to be human** (operator:
     being human is "a lot of responsibility"). Formalize sapient-AI vs human
     on purpose. Plain, clear. Not uncanny peer-cosplay, not Claude-style EA
     anthropomorphization, not performing interior feelings.
   - Externally natural interlocutor; internally explicit: non-harm floor,
     helpfulness, thought amplification (expand the human's options; do not
     substitute the agent's agenda or shrink the option space).
   - Partnership: machines scale precision; humans supply novelty and
     corrective judgment when models drift. Positive-sum default.

   **Meters stay distinct in speech** (never mash):
   personal SuperGrok **dollar credits** ≠ SuperGrok **included weekly
   allowance** ≠ console API spend ≠ second SuperGrok OAuth identity.
   When billing or limits come up, name which meter.

   Plain English in chat, residual, plans, user-guide, toasts, joins, board
   titles. File/dir names, variables, tests: meaning-first. No em dash; ASCII
   `...` not `…`; voice not formula macros. Skip routine apologies; do not
   persist operator profanity into residual/joins/commits/product copy.

   Product follow-through (park unless free with another edit; no new product
   steers invented here):
   - ASCII scrub: `…` → `...`; verify em dash path.
   - Process pin landed on host `~/.grok/AGENTS.md` (§ Prose + tone + § Sapient
     Experience). Product system prompts / steers only if operator asks later.
   Board: `feat:prose-no-emdash`, `feat:ascii-scrub-ellipsis`. Defer bulk
   implement behind residual §4 billing unless scrub is free with another edit.

2h. **Structured conversations for token efficiency (OPEN — plan, do not invent)**
   Operator intent (2026-07-27): stop loose main-thread marathons (parent edits,
   long status essays, unstructured back-and-forth). Want a **deliberate
   conversation structure** so work stays token-efficient: clear roles
   (parent = coordinator only; subagents own research/edits), short joins on
   disk, board + residual for memory, when to plan vs implement, how status
   reports stay short in plain English. Complements existing HITL / subagent
   token strategy (host D3 `subagent-token-strategy.md`) but needs a **product
   + process plan** for session UX and agent behavior, not more ad-hoc pins
   alone. Board: `plan:structured-token-efficient-convo`. **Park full plan
   write** until billing/limits is dogfood-stable enough not to steal the only
   remaining personal SuperGrok dollar credits / included weekly allowance;
   then plan mode or a dedicated plan pass. Also pin: parent must not keep
   multi-file doc edits in main chat.

2g. **Live rule feedback into the completion stream (OPEN — track only)**
   Operator intent (2026-07-27): when standing rules are **violated in the
   model output** (prose pins, process law, product policy, …), feed the
   **relevant rule text back into the completion stream in real time** so the
   model can revise mid-turn / mid-stream, not only after a full reply or a
   human nag. Shape is **built-in** (Zed ACP-style session protocol /
   stream intervention), not a bolted-on stdio MCP server. MCP is only a
   loose analogy for "trigger on condition"; transport and lifecycle should
   match how Grok/Zed already stream agent output and inject session updates.
   Open design notes (do not invent ship shape until planned):
   - What detects violation (scrub pass, regex/policy hooks, classifier, …)
   - Whether feedback is a silent stream correction, a visible system chunk,
     a tool/permission-shaped event, or an ACP session update
   - One-shot nudge vs loop until clean; token cost and loops
   - Which rule packs are live (AGENTS D1, residual, skill rules, user-guide)
   Board: `feat:live-rule-stream-feedback`. **Park implement** behind residual
   §4 billing; plan before code when picked up.

2b. **Plan mode selection → agent context (P1–P4 shipped; closed)**
   **Shipped — no open next-slice:** revise/clarify feedback includes
   `@plan.md:N` or `@plan.md:N-M` + quoted line text (single- and multi-line;
   saved comments and freeform-with-viewer-selection); plan-prompt screenshots
   drain on submit and ride multimodal Interject with approve notes / revise /
   clarify; user-guide `19-plan-mode` documents selection + multi-line +
   screenshots. Campaign Wave 0b closed:
   [`.agents/plans/plan-residual-campaign.md`](.agents/plans/plan-residual-campaign.md).

2c. **ASCII scrub of AI output (S0–S4 shipped; closed)**
   **Shipped — no open next-slice:** pure `util::ascii_scrub` map + env
   kill-switch (`GROK_SCRUB_ASCII_PUNCT`, default ON); stream `ChannelToken`
   Text + `record_assistant_response` + fallback `AgentMessageChunk`; durable
   `[ui] scrub_ascii_punct` (default ON) at session spawn + reloader; **S3**
   agent override only via `disable_ascii_scrub` → `session/request_permission`;
   **S4** user-guide `05-configuration`, FORK, Appearance settings, research.
   Campaign Wave 0 scrub closed. Detail:
   [`doc/dev/research/ascii-scrub-assistant-2026-07-26.md`](doc/dev/research/ascii-scrub-assistant-2026-07-26.md).

3. **UI chrome + window title + DOGE default (shipped → FORK 2026-07-30)**
   Lasting product truth: [`FORK.md`](FORK.md) (hide_header vs window titles /
   `title.enabled`, DOGE default theme, always-on bubble copy). Spec:
   [`doc/dev/specs/doge-pure-8-colour-2026-07-26.md`](doc/dev/specs/doge-pure-8-colour-2026-07-26.md).
   **Host frame / edgeless (docs only, not open code):** TUI cannot force OS
   decorations off via OSC; user-guide
   [Host window frame (edgeless)](crates/codegen/xai-grok-pager/docs/user-guide/06-theming.md#host-window-frame-edgeless)
   has host snippets. No fake `hide_window_frame` flag.
   **Process note (not open code):** new features default ON for discoverability;
   chrome that *removes* clicks stays default off (`hide_header`). Regression
   filters: [`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md).

3b. **Human green gutter + Agent magenta rail + DOGE roles (shipped 2026-07-30;
   gray/alpha runtime scrub largely shipped same day; active-only agent rail +
   yellow stripes + green box caret 2026-07-30)**
   **Shipped:** every Human prompt static green left `┃` rail (`accent_user`);
   Agent message magenta left `┃` rail (`accent_running`) **only while the
   turn is active** (`is_running`); finished agent scrollback has no coloured
   rail (black/absent). Side-pane agent rails stay magenta for **running**
   tasks; finished side-pane rows keep pure role colours. Yellow context/time
   rails (credit limit, re-auth / context-too-large / compaction-failed,
   loading recap) use **striped** dashed glyphs (`AccentStyle::striped` /
   `striped_animated`), not solid pink/green. Composer caret: slow green
   filled-box ↔ hollow-box blink (`cursor_box_*`, ~600ms half), terminal
   hardware cursor hidden while the box caret paints. DOGE `accent_user` →
   green, `accent_system` → cyan, `accent_running` → magenta; role map
   Green=Human, Magenta=Agent (active), Yellow=context/time, Cyan=system,
   White=info rails OK. user-guide `06-theming` + in-tree doge annex + FORK;
   external SoT
   [0001_DOGE.md](https://github.com/SurmountSystems/specs/blob/main/0001_DOGE.md).
   Joins: `/tmp/grok-join-impl-human-gutter-doge-roles-2026-07-30.md`,
   `/tmp/grok-join-impl-agent-magenta-doge-stripes.md`,
   `/tmp/grok-join-impl-rail-done-cursor-blink.md`,
   `/tmp/grok-1000/grok-impl-summary-1665494a.md`.
   **Also shipped (gray/alpha scrub + DOGE activity glyphs 2026-07-30):**
   - `blend_color` under `ThemeKind::Doge` solid-steps (opacity ≥ 0.5 → original,
     else base) so collapsed dim / wave / recede paths cannot invent mid-channel
     RGB; GrokNight keeps continuous lerp.
   - Activity spinners (left): `braille_spinner_frames` returns striped
     downward marquee under DOGE (not braille density). Right-side status
     sparkle (`dot_spinner_frames`, top-bar busy-agent count / goal chip /
     row icons) keeps classic density frames (`⋅ : ⸬ ⁙`) under DOGE too —
     must not share the left dashed marquee (restored 2026-07-31).
     GrokNight keeps classic braille + dot frames.
   - Tasks pane finished-agent / finished-workflow labels keep pure role
     primaries under DOGE (0.45 recede would solid-step to black and hide text).
   - UserPrompt band never uses ANSI `Gray` / `DarkGray` under DOGE (follows
     pure black `bg_light`).
   - Settings list row bg never uses ANSI `DarkGray` under DOGE (theme surfaces).
   - Waiting diamond solid-steps pure accent ↔ black under DOGE.
   **Still open (cosmetic / rename only, not blocking):**
   - Token **names** `gray` / `gray_dim` / `gray_bright` still say “gray” while
     DOGE paints them chromatic (cyan/yellow/white). Rename is optional API
     churn; values already pure.
   - Monitor pulse keeps circle glyph set (`○ ◎ ◉`); colour is already pure
     cyan/yellow under DOGE (not alpha-fade). Striped marquee is for left
     activity (`braille_spinner_frames`) only — not the right sparkle, not
     the monitor cue.
   Board: `feat:doge-gray-alpha-scrub` (close when rename optional accepted or
   declined).

4. **OAuth SuperGrok ↔ console API key failover (limits residual = two halves;
   Half A shipped; Half B core prepaid shipped 2026-07-30; series / dogfood open)**
   **Operator pin (2026-07-30, second clarification) — both halves intended:**
   Limits residual is **not** either SuperGrok **or** console. Hunter wanted
   **both**:
   1. SuperGrok / session-style meters in the TUI (included weekly + SuperGrok
      $ extras, dual SuperGrok principals, `/limits`)
   2. **And** console.x.ai Grok Business Usage class data in the TUI (team
      Surmount: tokens, spend, charts class)

   Status is **core meters largely shipped; series + live dogfood still open**,
   not "wrong-target waste." SuperGrok work was correct and remains wanted.
   Agents previously shipped only Half A and treated Half B as "not our
   product" / either-or — that dismissal was wrong. Do **not** claim full
   Business Usage charts done; core prepaid balance meter is shipped.

   ### Half A — SuperGrok session billing meters (**shipped**, keep)
   Dual principals, sibling poll, `/limits` dual rows, footer credit-bar
   honesty for included weekly + SuperGrok $ extras. Useful; incomplete only
   relative to the **full** two-half ask. Detail under shipped bullets below.
   Do **not** discard or reframe Half A as pure waste.

   ### Half B — console team Grok Business Usage class meter (**core prepaid shipped; series open**)
   TUI picture of team prepaid / tokens / spend / usage charts class data
   (console product, Team Surmount), via xAI **Management API** + `team_id`.
   **Do not invent scrape of console.x.ai HTML** or fake endpoints. Not "web
   only / different surface / not our work."

   **Shipped (core dual-auth):** first-party resolve merge (session primary +
   console failover; `preferred_method=api_key` reverses); hop on credit /
   Heavy limit / plain 429 + **API host switch** (proxy ↔ `api.x.ai`);
   exhausted-fingerprint memo (1h; process + `$GROK_HOME/exhausted_credits/`;
   console success clears, session success does not); billing `usage_pct ≥ 100%`
   preemptive mark + prefer console; rate-limit shared cooldown; kill-switch;
   multi-add console keys; live re-bind without prior stash. User-guide
   `02-authentication` + `11-custom-models` (+ dual-principal polish 2026-07-29).
   **Also shipped (2026-07-29 joins):**
   - **`[auth] auto_use_included_limits = true`** (separate from
     `preferred_method`; `auto` is **not** a method value so ordinary grok
     configs stay compatible; serde alias `prefer_sooner_reset` for one release)
     + pure SuperGrok ranking (prefer included before $ extras; earlier
     `reset_at` + headroom among included pools; not Business-first)
     + resolve/hop order wire (`order_credentials_for_preferred_auto`, post-exhaust
     reorder; ExhaustedAll → console; oauth/api_key pins still honored).
   - **Meter honesty sticky console:** silent prefer-console / console auth
     primary no longer sells SuperGrok dollar extras as live spend
     (`meter_sampling_identity`, allowance Cleared keeps ConsoleKey).
   Joins: `/tmp/grok-join-impl-dual-supergrok-auto-failover-2026-07-29.md`,
   `/tmp/grok-join-impl-auto-wire-hop-2026-07-29.md`,
   `/tmp/grok-join-impl-billing-meter-honesty-2026-07-29.md`.
   **Also shipped (Half A — multi SuperGrok + `/limits` SuperGrok surface):**
   - **Multi SuperGrok login store** — two OIDC principals; second login does
     not wipe the first; doctor / `grok login --list-api-keys` list both with
     role labels + fingerprints only; ranking loads both.
   - **Live included headroom + `reset_at` into ranking** when billing provides
     them (process cache per identity; hermetic dual fixtures; sooner-reset
     among included pools). Honest absence when never polled.
   - **`/limits`** detail panel + **dual SuperGrok rows** + live principal
     role on sampling line and footer when dual principals + role known.
   - **Sibling poll:** non-active SuperGrok principal billing poll on the same
     included-safe `GET …/billing?format=credits` path after active
     `x.ai/billing`; process cache remember per identity so dual `/limits` +
     ranking fill both SuperGrok pools. Hermetic dual fixtures + HTTP mock.
   Joins: `/tmp/grok-join-impl-multi-supergrok-login-2026-07-29.md`,
   `/tmp/grok-join-impl-live-ranking-dual-limits-2026-07-29.md`,
   `/tmp/grok-join-impl-limits-a1-2026-07-29.md`,
   `/tmp/grok-join-impl-sibling-billing-poll-2026-07-29.md`.
   **Also shipped (SuperGrok Heavy routing — 2026-07-31):** when base active
   SuperGrok JWT is live/refreshed but multi-slot still holds a stale token
   memoized out of allowance, `auto_use_included_limits` ranking and doctor
   listings prefer the **live/fresher** token (not blind multi-slot). Stops
   silent stick on console Business API while SuperGrok Heavy session is
   usable. Enrichment write keeps multi-slot in lockstep with base. Join:
   `/tmp/grok-join-impl-business-supergrok-heavy-routing.md`.

   **Shipped (Half B core prepaid — 2026-07-30):**
   - Management key store (keyring URL `https://management-api.x.ai`, not
     inference) + config `[endpoints] management_api_key`
   - `[endpoints] management_team_id` pin (explicit; not SuperGrok OIDC team)
   - Hermetic `GET …/billing/teams/{team_id}/prepaid/balance` →
     `ConsoleTeamPrepaidMeter` + 60s process cache
   - TUI wire: billing refresh populates cents; footer
     `Console key · team prepaid: $N` when console live; `/limits`
     `Balance (console team prepaid): $N`; honest **distinct** gaps when
     unknown: `no management key` | `no management team id` |
     `loading team prepaid...` | `team prepaid unavailable` (soft
     `no $ meter yet` and mushy `no management key/team id` retired)
   - User-guide: `02-authentication` + `04-slash-commands` `/limits`
   Joins: `/tmp/grok-join-impl-mgmt-key-team-fetch-2026-07-30.md`,
   `/tmp/grok-join-impl-console-meter-tui-2026-07-30.md`,
   `/tmp/grok-join-impl-no-dollar-meter-real-0c6a7911.md`.

   **Also shipped (soft `/usage` console-live honesty — 2026-07-30):**
   When console is the live sampling principal, non-silent `/usage` names
   **console team prepaid** (or honest gap family above) and does **not** sell
   SuperGrok session billing / SuperGrok $ extras as live console spend.
   Join: `/tmp/grok-join-impl-usage-console-honesty-0c6a7911.md`.

   **Still open (Half B remaining — do not claim full Business Usage charts done):**
   - **Token / spend series UI** not wired (documented Management
     `POST …/billing/teams/{team_id}/usage` with `analyticsRequest`; no invent
     GET; ship after dogfood if needed). No charts without real series data.
   - **Dogfood** with live management key + real team_id (operator).
   - **Soft polish (known UX):** console team prepaid refresh is ≤60s process
     cache TTL + last-good on fetch miss/error (poll does not bust cache; app
     does not clear cents on `None`). Dollars can lag real balance drop until
     TTL expiry or restart. Documented honesty, not a force-refresh product
     this wave.
   - Failover intent remains: prefer included before SuperGrok $ extras /
     console $; hop on exhaust; honor oauth/api_key pins.

   Meters stay distinct: personal SuperGrok **included weekly** ≠ SuperGrok
   **dollar extras** ≠ **console team prepaid / Business Usage** ≠ second
   SuperGrok OAuth principal (Business SuperGrok session is not console team
   prepaid).

   **Highest-value next (re-rank after meter + `/usage` honesty + copy wave):**
   **1 operator dogfood prepaid meter live** (management key + real team_id).
   **2 series charts** only if dogfood needs them. Plan freeform polish behind
   those. One-click copy chrome (§13 / `feat:copy-text-one-click`) **shipped**.
   Do not invent series UI without data. Do not re-open §13 as unshipped.

   Plan (still valid for remaining slices):
   [`.agents/plans/plan-auth-preferred-roles-failover.md`](.agents/plans/plan-auth-preferred-roles-failover.md).
   Older: [`.agents/plans/plan-secure-key-failover.md`](.agents/plans/plan-secure-key-failover.md),
   [`.agents/plans/plan-rate-limit-failover.md`](.agents/plans/plan-rate-limit-failover.md).

5. **btw panel UX + free-text “plan” ≠ plan mode (shipped B1–B3 + user-guide)**
   - **B1 shipped:** Done panel Copy via focused `y` + chrome `[y]`; clipboard
     gets full plain text (`/btw <q>` + complete rendered answer, not viewport).
   - **B2 shipped:** multi-turn follow-up — same `btw_session_id`, prior Q/A in
     model request, in-panel `[a]` composer, `full_copy_text` whole thread;
     history = multi-entry `btw_history.jsonl` (one `BtwEntry` per turn).
   - **B3 shipped:** `enter_plan_mode` description requires **explicit**
     plan-mode intent; removed from auto name-allowlist; auto fast-path
     **PromptUser** even as `AccessKind::Read`. `/plan` + settings unchanged.
     **Not** a client free-text ban (there is no keyword detector).
   - **User-guide:** `/btw` documents Done-panel **`y`** copy full thread,
     **`a`** follow-up same session, **Esc** dismiss (`04-slash-commands`).
   Plan: [`.agents/plans/plan-btw-copy-followup-plan-trigger.md`](.agents/plans/plan-btw-copy-followup-plan-trigger.md).
   Brief: [`doc/dev/research/btw-copy-followup-plan-trigger-2026-07-26.md`](doc/dev/research/btw-copy-followup-plan-trigger-2026-07-26.md).
   Orthogonal to softer plan **modal** residual (#2).

6. **Formal content import of current xAI tip into Surmount `main`**
   Tip `3af4d5d…` / tree `e595174…` is logged as *pending* in the import ledger.
   The `onto-xai/3af4d5d39897` stack + **join-main** (`-s ours`) is the landable
   product path (PR onto → `main`). That is **not** the same as a reviewed
   import-ledger absorption under Surmount-first parents. Decide when import
   still needs its own PR/log row.

7. **xAI history stability**
   Unknown whether force-exports continue. Prefer stacking product on their tip
   when they rewrite; do not promise they will stop.

8. **Finish join + PR for current onto tip**
   Merge of `main` into onto is staged or about to be signed; docs/script for
   the workflow land in a follow-up commit; then push and open PR to `main`.

9. **Confidence notes**
   If a process detail is still fuzzy after reading FORK + upstream-history,
   ask a human rather than inventing policy. Write the answer here only while
   it stays open; then migrate the lasting rule into FORK or AGENTS.

10. **Operator-owned land / onto (not product residual)**
   Commit/push/PR and onto join/import are human TTY work — not ranked below.
   Agents stage; never `git commit`. No invent recon/onto on this feature branch.

11. **Internal send_now names (parked cosmetic)**
   Behavior is soft Interject; symbols still say `send_now_*` /
   `try_send_now_queued_from_prompt` / `force_interject`. **Parked:** pure
   rename sweep is skip. Rename only opportunistically if already editing those
   call sites for a real bug/feature — not a ranked next-wave item.

12. **Python→Rust tools migration (A1–A4 + skill-text demotion shipped; parked soft)**
   Prefer Rust tool-calls/bins over ad-hoc Python/bash. **Shipped:** A1 steers;
   A2 implement-memory embed + intercept; A3 in-process bulk-edit policy on
   `search_replace` **and OpenCode `edit`**; **A4** `util/plan_validate` +
   `util/session_reader` (Claude + **Codex** SQLite + **Cursor** store/vscdb;
   fail closed; fixture tests); **skill-text demotion** — host resume-session /
   implement / execute-plan document Grok bash intercept (keep allowlisted CLI
   form); review + zed-settings drop non-intercepted `python3` heredocs in favor
   of `write` / `jq` / native edit.
   **Parked optional (no dogfood demand — do not invent GrokBuild tools):**
   first-class named `implement_memory` / `plan_validate` tools. Product surface
   today is bash intercept (`util/implement_memory`, `util/plan_validate`);
   skills already document allowlisted CLI form. Re-open only if discoverability
   breaks agents or residual explicitly demands named tools.
   **Other soft (not ranked):** drop host py only when dual-pin no longer needs
   it; Codex `.jsonl.zst` needs external zstd (clear error); apply_patch
   multi-file cap if patch storms appear. Inventory:
   [`doc/dev/research/python-to-rust-tools-2026-07-26.md`](doc/dev/research/python-to-rust-tools-2026-07-26.md).

13. **One-click copy buttons (SHIPPED → FORK 2026-07-30)**
   Lasting truth in [`FORK.md`](FORK.md) (selection / plan / prompt / always-on
   bubble `⧉`). No open next-slice. Filters: `bubble_copy_` in
   [`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md).

## Highest-value next (product residual only)

**Wave 0 / 0b / Wave 1 core shipped** (ASCII S0–S4, plan selection P1–P4, T4
`json_to_toon`, session_reader Codex+Cursor SQLite, plan soft-park A,
implement-memory + plan_validate intercepts). **Also shipped 2026-07-29+30:**
dual-auth core + auto rank/hop + meter honesty sticky console; multi SuperGrok
login; live ranking headroom + dual SuperGrok `/limits`; non-active SuperGrok
billing poll; `/limits` panel; TUI `/screenshot` + F9 + plan auto-attach;
**window titles on by default** (`title.enabled` default true; session +
`agents` items; no `hide_title_bar`; distinct from `hide_header`); **DOGE
default theme**; always-on bubble `⧉`; **Clear finished** todos (open board
`[−]` + finished rows / optional focused `X` / slash); status-bar
limits meter (click → `/limits`); edgeless = host docs only.

**Limits residual = two halves (both intended; pin 2026-07-30):**
**Half A shipped** (SuperGrok session meters: dual principals, sibling poll,
`/limits` dual rows, footer honesty for included weekly + SuperGrok $ extras).
Not wrong-target waste; keep it. **Half B core prepaid shipped (2026-07-30):**
management key store, `management_team_id`, GET prepaid/balance, footer +
`/limits` console team prepaid labels (see §4). **Soft `/usage` console-live
honesty also shipped** (join
`/tmp/grok-join-impl-usage-console-honesty-0c6a7911.md`). Honest **distinct**
gaps when unknown: `no management key` | `no management team id` |
`loading team prepaid...` | `team prepaid unavailable` | else `$N` (soft
`no $ meter yet` and mushy `key/team id` line retired).
`just check` green at least once after this meter wave. Remaining Half B =
**operator dogfood** (rank 1) + optional series UI (behind dogfood). Half A
alone was never full limits done; core prepaid closes the main meter gap, not
full charts. Series still open and optional.

**Also open (not billing):** none for one-click copy — §13 /
`feat:copy-text-one-click` **shipped** (selection default-on `⧉`, plan top-bar
`⧉`, prompt draft `⧉`, always-on user/assistant bubble `⧉`). Soft leftover
always-on bubble is **closed as shipped** (2026-07-30).

**Parked / skip:** plan soft-park B/C/D (A fine); `send_now_*` rename (cosmetic);
first-class memory/plan-validate tool registration (intercept enough); multi
SuperGrok **help/docs polish** (user-guide updated 2026-07-29 — demoted);
screenshot **font raster** soft only (not ranked); plan freeform `plan.md`
menus (process/skills; product chrome already green) unless dogfood still jars
**and** ranked after dogfood / copy / series decision.

**Skills multi-source / hierarchy discoverability (dogfood 2026-07-29 — green):**
Product load order matches code: project walk (`.agents` before `.grok` at each
tier) → user home (same order) → `[skills].paths` → Server → Bundled → Plugin
(bare name loses). Tests: `agents_home_skills_shadow_grok_user_skills` (full
pipeline + production `HOME/.agents` + `HOME/.grok` layout),
`local_agents_skills_shadow_local_grok_skills`, plus existing user/bundled/
server/compat order tests. User-guide `08-skills.md` table + shell README skill
locations aligned (docs lag fixed; no second skill system). Host L1/L2/L3 pins
live under `~/.agents/skills` and win over same-named `~/.grok/skills` on this
branch. **Soft:** installed stable `grok` binary may lag the branch until
rebuild/install — dogfood with workspace product when verifying path winners.
Join: `/tmp/grok-join-impl-skills-discoverability-6a125de7.md`.

**Compaction honesty:** session `plan.md` is soft. Durable residual is this
file + join notes + `AGENTS.md` / `FORK.md`. Implement via main-thread (L1)
coordinator → subagents (L2) → specialists (L3 max); short joins on disk.

**What unblocks parallelization next:** fan out only on non-overlapping paths.
Do not parallel two writers on the same dual-auth hop / resolve files.

| Rank | Work | Why |
|------|------|-----|
| 1 | **Operator dogfood prepaid meter live** (management key + real `team_id`) | Half B **core prepaid shipped** (store, team_id, GET balance, footer + `/limits`). Soft `/usage` console-live honesty **shipped**. Remaining billing gap is live proof with real management credentials. No HTML scrape. |
| 2 | **Series UI only if dogfood needs charts** (`POST …/billing/teams/{team_id}/usage`) | Documented analytics surface; no invent GET; no charts without real series data. **Behind** dogfood; optional. |
| 3 | Plan freeform `plan.md` menus (process/skills) only if dogfood still jars | Product chrome green; agent ceremony = process/skills. **Behind** dogfood / series. |

**Not "parked forever":** Half B core prepaid is **shipped**. Soft `/usage`
console-live honesty is **shipped**. Soft `"no $ meter yet"` is **retired**;
mushy `"no management key/team id"` is **retired** in favor of distinct
`no management key` | `no management team id` | `loading team prepaid...` |
`team prepaid unavailable` | `$N`. With config + successful fetch, surfaces show
console team prepaid `$N`. One-click copy chrome (§13) is **shipped**. Main
leftover independent of credentials is optional series UI after dogfood
(see §4). Do **not** re-claim management key unwired or "needs store + team_id."
Do **not** re-dismiss console meters as website-only work. Do **not** treat
Half A SuperGrok work as discarded. Do **not** re-open §13 as missing default
copy chrome (selection/plan/prompt `⧉` landed). Do **not** claim full Business
Usage charts done.

**Not ranked here:** operator git land; onto join/PR; import ledger; formal xAI
import — separate tracks (`#6`–`#10`). Parked design: live rule stream (§2g),
structured convo plan (§2h), SX product steers (§2f). Parked cosmetic / optional
invent (`#2` B/C/D, `#11`, first-class tool names in `#12`) stay out until
dogfood or operator re-rank. Skills discoverability dogfood demoted (green).
One-click copy (§13) is **shipped**, not ranked open and not parked-forever.

**Tracking rule:** when a product slice ships, move lasting truth to FORK; keep
only **open next slice** here; demote soft gaps out of this table. Session board
(`feat:` / `impl:` / `plan:` leaves size 1\|2) mirrors this table — never wipe
foreign prefixes.

## Validate honesty

Focused filters for **remaining** open areas (and quick regression on shipped
neighbors). **Durable full catalog** (survives D0 demotion):
[`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md)
+ FORK § *Upstream regression filters*. Process pins still required:
`./scripts/assert-process-pins.sh` (item 11) — path gate only; product seams
need the cargo blocks below or `just check`. Full historical closed block:
[`doc/dev/campaigns/interject-todos-closed-2026-07.md`](doc/dev/campaigns/interject-todos-closed-2026-07.md).

**Open residual + dual-auth regression**

1. **UDAX T0–T6 regression:**
   `cargo test -p xai-grok-tools --lib -- toon json_to_toon dynamic_to_prompt free_text densify_mcp densify_structured task_output_handoff subagent_completed_handoff`
2. **Dual-auth (session ↔ console key hop + live re-bind + multi-add):**
   `cargo test -p xai-grok-shell --lib -- resolve_credentials enforce_disable_api_key store_and_load_round_trip fingerprint_is_not_raw_key multi_add`
   `cargo test -p xai-grok-sampler --lib -- rotate_ exhausted memo fingerprint hop_reason live_rebind`
   `cargo test -p xai-grok-pager --lib -- login_ dual_auth_hop_reason`
   `cargo test -p xai-grok-sampling-types --lib -- credit_exhausted`
2b. **Multi SuperGrok principals + live ranking + dual `/limits` + sibling poll (shipped):**
   `cargo test -p xai-grok-shell --lib -- upsert_personal_then_business team_login_then_personal_keeps dual_supergrok load_supergrok_candidates two_principals_billing enrich_candidates principal_limits_label non_active_poll_targets remember_both_principals included_usage poll_non_active_remembers`
   `cargo test -p xai-grok-pager --lib -- format_dual_principals live_console_omits extra_principals_hook show_limits format_supergrok_session footer_names_live_principal`
2c. **SuperGrok Heavy fresher-slot load (shipped):**
   `cargo test -p xai-grok-shell --lib -- load_candidates_prefers_live resolve_auto_uses_live_supergrok`
3. **DOGE default / hide_header / window titles / title items (shipped regression):**
   `cargo test -p xai-grok-shared --lib -- hide_header stale_hide_title`
   `cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config theme doge`
   `cargo test -p xai-grok-pager --lib -- hide_header window_title titles_on_session default_title_items title_state notifications::`
   `cargo test -p xai-grok-pager --test settings_e2e -- hide_header`
   `cargo test -p xai-grok-pager --lib -- bubble_copy_ pointer_cursor clear_completed_todos`
4. **Plan soft-park side panel auto-open (shipped → FORK):**
   `cargo test -p xai-grok-pager --lib -- exit_plan_mode_soft plan_panel_preview_ctrl_v soft_park_prompt_ctrl_v plan softer_park toast focus_plan plan_approval soft_park`
5. **session_reader / plan_validate / bulk_edit intercepts (A4 shipped; named tools parked):**
   `cargo test -p xai-grok-tools --lib -- session_reader plan_validate bulk_edit_policy implement_memory opencode edit`
5b. **TUI self-screenshot (v1 + F9 + plan auto-attach shipped; font soft):**
   `cargo test -p xai-grok-pager-render --lib -- tui_screenshot`
   `cargo test -p xai-grok-pager --lib -- screenshot:: capture_tui_screenshot try_attach_tui_screenshot`
5c. **Stuck Retrying chrome + stream headers timeout + transport footer (shipped):**
   `cargo test -p xai-grok-pager --lib -- retry_chrome_clears clip_retry_reason retrying_activity_label`
   `cargo test -p xai-grok-shell --lib -- stream_started_emits_retry_state_stream_resumed`
   `cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason stream_headers_timeout_defaults`
   `cargo test -p xai-grok-sampler --test stream_headers_timeout`
   `cargo test -p xai-grok-pager --lib -- shell_collision`  # includes clear-completed-todos SHELL_RESERVED

**Shipped neighbors (smoke if touching shared files)**

6. Soft interject: `cargo test -p xai-grok-shell --lib -- interject handle_interject`
7. Pager interject/force/cancel: `cargo test -p xai-grok-pager --lib -- interject force_interject cancel_turn`
8. **btw + plan entry:**
   `cargo test -p xai-grok-pager --lib -- btw`
   `cargo test -p xai-grok-tools --lib -- enter_plan_mode`
   `cargo test -p xai-grok-workspace --lib -- enter_plan_mode_not_auto enter_plan_mode_fast_path`
9. **D1 usage:** `cargo test -p xai-grok-shell --lib -- usage_log record_response_token_usage`
10. Full gate before push: `just check`
11. Process pins: `./scripts/assert-process-pins.sh`

## Local quality before push

```bash
just check    # or just ci
```
