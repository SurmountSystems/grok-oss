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

- **Stuck Retrying / network-switch graceful (shipped → FORK; dogfood rebuild):**
  product truth in [`FORK.md`](FORK.md) (StreamResumed soft-reconnect, not
  zombie Waiting for response; 120s headers timeout; cancel-aware cooldown;
  `timed out` / `connection interrupted` + `· next try in Ns`). Soft not
  shipped: phase-timer "since retry"; live countdown ticks. **Dogfood** after
  rebuild if the stable binary lags (`Credits used:` in screenshots was
  already percent-only in source). Report:
  `/tmp/grok-join-impl-retry-network-graceful.md`.

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

   Plain English in chat, residual, plans, user-guide, toasts, reports, board
   titles. File/dir names, variables, tests: meaning-first. No em dash; ASCII
   `...` not `…`; voice not formula macros. Skip routine apologies; do not
   persist operator profanity into residual/reports/commits/product copy.

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
   (parent = coordinator only; subagents own research/edits), short reports on
   disk, board + residual for memory, when to plan vs implement, how status
   reports stay short in plain English. Complements existing HITL / subagent
   token strategy (host D3 `subagent-token-strategy.md`) but needs a **product
   + process plan** for session UX and agent behavior, not more ad-hoc pins
   alone. Board: `plan:structured-token-efficient-convo`. **Park full plan
   write** until billing/limits is dogfood-stable enough not to steal the only
   remaining personal SuperGrok dollar credits / included weekly allowance;
   then plan mode or a dedicated plan pass. Also pin: parent must not keep
   multi-file doc edits in main chat.

2i. **Multi-track / also guard (OPEN — product honesty; process pins not enough)**
   Operator call-out (2026-08-01): parent demoted in-flight limits work to
   pending when a second ask (console drain screenshots) arrived — abandoned
   the first track without needing kill. Process law alone failed again.
   **Wanted product (plain):** session board binds in-flight subagents to
   todos; parent cannot demote a live track without explicit cancel; optional
   sticky "N agents running" (or equivalent) when a new user message arrives
   while agents are live. Process dual-pin already stricter (host
   `~/.grok/AGENTS.md` § *Multi-track: prose is not enough*; project
   `AGENTS.md` bullet). **Do not claim mechanical guard shipped.** Board:
   `feat:multi-track-also-guard`. Rank: open product honesty, not closed.

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

4. **OAuth SuperGrok ↔ console API key failover (limits residual = two halves
   + limits-first campaign; core product largely shipped 2026-08-02)**
   **Operator pins:**
   - **Limits before credits** (always). Design A: while SuperGrok included
     weekly has headroom, omit console ApiKey from the chain. Do not hop to
     console to "fix" team Usage $.
   - **Both halves intended** (2026-07-30): not either SuperGrok **or**
     console. Hunter wanted **both**:
     1. SuperGrok / session-style meters in the TUI (included weekly + SuperGrok
        $ extras, dual SuperGrok principals, `/limits`)
     2. **And** console.x.ai team Usage class data in the TUI (team Surmount:
        prepaid, spend class, optional charts)

   Status is **core meters + limits-first Slices 1/3/4 shipped; Slice 2 dogfood
   + residual-edges wave (soft honesty + bare-resolve audit) shipped 2026-08-02;
   operator-edges residual wave same day: C4 ticket evidence package ready,
   prepaid TTL polish shipped, F1b soft product close-out done; live multi-poll
   still blocked; C4 still FAIL (branch 2b, server-side); product client invent
   for limits-first largely exhausted**, not "wrong-target waste." SuperGrok
   work was correct and remains wanted. Do **not** invent SuperGrok included
   debit (C4) as proven: product honesty holds; server debit under load is still
   not a clean pass (weak log 65→66, Build still flat 54; multi-poll attempt
   auth-failed / cold process, `flat_poll` absent). Do **not** claim full
   Business Usage charts done; core prepaid + postpaid OAuth/API class meters
   are shipped.

   ### Half A — SuperGrok session billing meters (**shipped**, keep)
   Dual principals, sibling poll, `/limits` dual rows, footer credit-bar
   honesty for included weekly + SuperGrok $ extras. Useful; incomplete only
   relative to the **full** two-half ask. Detail under shipped bullets below.
   Do **not** discard or reframe Half A as pure waste.

   ### Half B — console team Usage class meters (**core prepaid + M3 postpaid
   shipped; series charts still optional**)
   TUI picture of team prepaid / postpaid OAuth vs API class / optional token
   spend charts (console product, Team Surmount), via xAI **Management API** +
   `team_id`. **Do not invent scrape of console.x.ai HTML** or fake endpoints.
   Not "web only / different surface / not our work."

   **Shipped (core dual-auth):** first-party resolve merge (session primary +
   console failover; `preferred_method=api_key` reverses); hop on credit /
   Heavy limit / plain 429 + **API host switch** (proxy ↔ `api.x.ai`);
   exhausted-fingerprint memo (1h; process + `$GROK_HOME/exhausted_credits/`;
   console success clears, session success does not); billing `usage_pct ≥ 100%`
   preemptive mark (Slice 4: do not mark / clear mark while SuperGrok $ extras
   known positive under auto_use); rate-limit shared cooldown; kill-switch;
   multi-add console keys; live re-bind without prior stash. User-guide
   `02-authentication` + `11-custom-models` (+ dual-principal polish 2026-07-29).
   **Also shipped (2026-07-29 joins; order refined 2026-08-02 Slice 4):**
   - **`[auth] auto_use_included_limits = true`** (separate from
     `preferred_method`; `auto` is **not** a method value so ordinary grok
     configs stay compatible; serde alias `prefer_sooner_reset` for one release)
     + pure SuperGrok ranking (prefer included before $ extras; earlier
     `reset_at` + headroom among included pools; not Business-first)
     + resolve/hop order wire (`order_credentials_for_preferred_auto`):
     included headroom → SuperGrok only (Design A, console omitted);
     included full + SuperGrok $ extras > 0 → SuperGrok primary, console
     failover (after-burner); included full + extras 0/unknown → console
     primary; `preferred_method=api_key` still pins console first.
   - **Meter honesty sticky console:** silent prefer-console / console auth
     primary no longer sells SuperGrok dollar extras as live spend
     (`meter_sampling_identity`, allowance Cleared keeps ConsoleKey).
   Joins: `/tmp/grok-join-impl-dual-supergrok-auto-failover-2026-07-29.md`,
   `/tmp/grok-join-impl-auto-wire-hop-2026-07-29.md`,
   `/tmp/grok-join-impl-billing-meter-honesty-2026-07-29.md`,
   `.agents/joins/impl-slice4-extras-before-console-2026-08-02.md`.
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
   usable. Enrichment write keeps multi-slot in lockstep with base. Report:
   `/tmp/grok-join-impl-business-supergrok-heavy-routing.md`.
   **Also shipped (limits-first campaign — 2026-08-02):**
   - **Slice 1 — poll history / flat honesty:** process ring of S1 credits
     samples per SuperGrok identity; pure flat detector
     (`included_debit_unproven`); wires
     `LimitsSnapshot.flat_poll_unproven_debit` on `/limits` and
     `limits --json` from real history (not test-only setter). Optional
     `billing: poll_delta` log when included % / Build % / extras cents step.
     Report: `.agents/joins/impl-slice1-poll-history-2026-08-02.md`.
   - **Slice 3 — M3 postpaid invoice preview:** Management
     `GET …/billing/teams/{team_id}/postpaid/invoice/preview`; aggregate
     OAuth vs API class cents; surface under console meter family in
     `limits --json` (`teamPostpaid*Usd` / `teamPostpaidGap`); C6 honesty
     when SuperGrok live and OAuth postpaid dominates (session can still
     move team Usage dollars without console key live). Does **not** change
     Design A or invent dollars without Management response. Prepaid $ and
     SuperGrok extras stay distinct. Report:
     `.agents/joins/impl-slice3-m3-postpaid-2026-08-02.md`.
   - **Slice 4 — SuperGrok $ extras before console (C5 after-burner):** when
     `auto_use_included_limits` is on and included is full, positive SuperGrok
     $ extras keep SuperGrok session primary (console failover only); extras
     0/unknown → console primary as before. Memo does not mark SuperGrok out
     of allowance while known positive extras remain. Report:
     `.agents/joins/impl-slice4-extras-before-console-2026-08-02.md`.
   - **Residual-edges wave — branch 2b soft honesty polish (2026-08-02):**
     dynamic flat-poll note (names SuperGrok included % always; Build /
     SuperGrok $ extras only when those meters were observed flat on the
     window); **Grok Build product usage: N% used** on `/limits` and non-silent
     `/usage` when the principal has wire %; C6 copy allows Team Usage $ to
     move without proving included weekly moved (console not live); doctor
     dual-auth line pins extras-before-console after-burner; sibling Build
     plumbing for dual principals. Does **not** invent C4 debit or flip
     default `auto_use`. Report:
     `.agents/joins/impl-branch-2b-honesty-2026-08-02.md`.
   - **Residual-edges wave — bare resolve / console-edge audit (2026-08-02):**
     `ModelsManager::sampling_config`, subagent model override (fail-closed
     when config missing and SuperGrok session live), and
     `resolve_model_to_sampling_config` all use
     `resolve_credentials_preferring_with_rank` with live preferred +
     `auto_use_included_limits`. Closes bare-`resolve_credentials` landmines
     that could queue console while SuperGrok included still had headroom.
     Public Imagine/STT hosts, BYOK/own-credentials, OpenRouter, and
     `preferred_method=api_key` remain **credential / host path** exceptions
     (not SuperGrok-first resolve); that is **not** an intentional rate-limit
     skip (see Phase R under Highest-value next). Did **not** flip default
     `auto_use` in that join (default later shipped 2026-08-03). Report:
     `.agents/joins/impl-bare-resolve-console-edge-audit-2026-08-02.md`.

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
   Report: `/tmp/grok-join-impl-usage-console-honesty-0c6a7911.md`.

   **Also shipped (Token Economy full product — 2026-08-03):** implement-effort
   policy under economic mode (ceiling 3 / desired 2 / all implement entry
   paths / clamp+toast); free SuperGrok period linear-burn pacing on
   credit/status + `/limits` + `/usage`; double-entry local vs Management on
   `/spend` and `/limits` section; durable **`$GROK_HOME/grok_oss.db`**. Report:
   [`.agents/joins/impl-token-economy-full-2026-08-03.md`](.agents/joins/impl-token-economy-full-2026-08-03.md).
   Lasting bullet: [`FORK.md`](FORK.md). **Operator dogfood only** (rebuild +
   management key for remote book) — not open code residual.

   **Still open (limits-first + Half B remaining — do not invent C4 debit):**
   - **C4 SuperGrok included debit still FAIL / branch 2b (server-side;
     product honesty held, not "fixed" debit; ticket evidence package ready):**
     joins
     [`.agents/joins/slice2-dogfood-g4-2026-08-02.md`](.agents/joins/slice2-dogfood-g4-2026-08-02.md),
     [`.agents/joins/live-limits-recheck-2026-08-02.md`](.agents/joins/live-limits-recheck-2026-08-02.md),
     [`.agents/joins/impl-branch-2b-honesty-2026-08-02.md`](.agents/joins/impl-branch-2b-honesty-2026-08-02.md),
     [`.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md`](.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md)
     (server ticket brief: identity + timestamps + flat series; **no invent
     debit**). Morning dogfood under heavy SuperGrok session traffic: included
     **65%**, Grok Build `productUsage` **54%**, SuperGrok $ extras **$100.29**
     flat. **C1 / C3 pass** for this product path (`liveSampling` SuperGrok,
     `console.isLive=false`). Later same-day log multi-sample: included
     **65→66** and GrokChat **11→12** once; **Build stayed 54**; extras still
     **$100.29**. Treat as weak / laggy / coarse-% evidence, **not** a clean
     C4 close. Shell tip recheck sparse (billing timeouts). Residual **2b**:
     server lag / coarse % / no proven controlled debit of this principal's
     included pool (Build especially). Keep Design A honesty; do **not** hop
     to console to "fix" included. Soft honesty surfaces (dynamic flat note,
     Build %, C6) are **shipped**; they do not invent a server debit. **C5**
     Slice 4 is **code-only** (not live-proved; included never hit ≥ 100%).
     Filing the ticket is **human / xAI**, not more client invent. **Do not
     invent SuperGrok debit.**
   - ~~**Default `auto_use_included_limits=true` for new installs**~~
     **shipped 2026-08-03** (operator approved; empty/new config defaults true;
     explicit false preserved; doctor + user-guide). Report:
     [`.agents/joins/impl-item1-default-prefer-free-allowance-2026-08-03.md`](.agents/joins/impl-item1-default-prefer-free-allowance-2026-08-03.md).
   - **Optional live multi-poll flat note** when SuperGrok **session billing is
     healthy** and the process is **long-lived** enough for multi-sample
     history. Soft product path is shipped. Live attempt
     [`.agents/joins/live-multipoll-flat-note-2026-08-02.md`](.agents/joins/live-multipoll-flat-note-2026-08-02.md):
     two spaced `limits --json` polls, byte-identical; SuperGrok included
     **66%** / extras **$100.29** / prepaid **$340** / `console.isLive=false`;
     billing auth failed (`Invalid or expired credentials`); `flat_poll*` and
     Build % **absent**. Cold / auth-fail process ≠ debit proof and ≠ flat-note
     live surface. Retry only when session billing poll succeeds.
   - ~~**Token / spend series + team default credits (Item 5 / former M6)**~~
     **shipped 2026-08-03** (see bullet above). Optional richer series **charts**
     only if dogfood still wants more than the shipped block; do **not** fold
     default credits into prepaid `$N`.
   - **TUI force-refresh parity with CLI shipped (2026-08-02):** explicit TUI
     `/limits` open (and `/limits --json`) force-busts Management
     prepaid+postpaid process caches then silent-FetchBilling (same class as
     CLI `grok limits`). Background FetchBilling still honors ≤60s TTL +
     last-good. Report:
     `.agents/joins/impl-tui-limits-force-refresh-2026-08-02.md`.
   - **Bare resolve dual-auth path (soft remaining only):** primary
     landmines **closed** with TDD (join above). Some surfaces still use
     public Imagine / STT hosts, BYOK / own-credentials, OpenRouter, or an
     explicit `preferred_method=api_key` pin and therefore do **not** go
     through SuperGrok-first credential resolve. That is a **credential /
     host path** fact, **not** permission to skip shared rate-limit cooldowns.
     **Phase R (open):** rate limits by API type must cover Imagine, voice,
     and BYOK paths; they are **not** an intentional rate-limit skip. Prior
     audit note: `.agents/joins/console-bypass-paths-code-audit-2026-08-02.md`.
   - **Multiproc SuperGrok billing + Management shared cooldowns (shipped
     2026-08-03):** SuperGrok session billing and Management API HTTP paths
     wait on / observe the flock JSON shared rate-limit store under
     `$GROK_HOME/rate_limits/`. Report:
     [`.agents/joins/impl-shared-rate-limit-billing-management-2026-08-03.md`](.agents/joins/impl-shared-rate-limit-billing-management-2026-08-03.md).
     **Not** claimed: full Phase R coverage of Imagine / voice / BYOK /
     inference by API type (still open).
   - **Durable multi-process SuperGrok included poll history (shipped
     2026-08-03):** ring under `$GROK_HOME/included_poll_history/` so flat-poll
     series survives process restart. Report:
     [`.agents/joins/impl-durable-included-poll-history-2026-08-03.md`](.agents/joins/impl-durable-included-poll-history-2026-08-03.md).
   - ~~**Item 5 — spend series + team default credits line**~~ **shipped
     2026-08-03:** Management usage series (documented POST) on explicit
     `grok limits` / limits surface; team default credits as its **own** line
     (never folded into console team prepaid `$N`). Report:
     [`.agents/joins/impl-item5-spend-series-default-credits-2026-08-03.md`](.agents/joins/impl-item5-spend-series-default-credits-2026-08-03.md).
   - **F1b attribution soft residual (product honesty complete — close-out):**
     browser team API Usage **$547.87** while SuperGrok ~65% / flat extras was
     explained as team **postpaid OAuth / Grok Build** class (not SuperGrok
     included debit; not secret console-key primary). M3 + C6 shipped;
     doctor/limits/usage no longer sell OAuth Usage $ as included weekly.
     Close-out join (green unit evidence, no code this wave):
     [`.agents/joins/impl-f1b-attribution-soft-2026-08-02.md`](.agents/joins/impl-f1b-attribution-soft-2026-08-02.md).
     Soft leftover only: browser $547 vs M3 ~$208 class totals (window /
     composite); optional live re-fetch. Does **not** re-open product honesty
     or rank policy. Evidence joins:
     `.agents/joins/console-api-usage-547-evidence-2026-08-02.md`,
     `.agents/joins/console-burn-one-turn-investigation-2026-08-02.md`.
   - **Live prepaid dogfood done (2026-08-02):** management key + real
     team_id path works; product **$340** matches prepaid `total.val`
     (see field map below). Dashboard ~$1317 is a different surface
     (defaultCredits / composite), not a second prepaid field to merge.
   - **Soft prepaid TTL / force-refresh polish shipped (2026-08-02):** process
     cache still ≤60s (`CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS`; prepaid
     alias kept) for TUI background polls; app still keeps last-good cents on
     fetch `None`. **Force path:** explicit `grok limits` collect **and** TUI
     `/limits` open bust prepaid+postpaid process caches via
     `clear_console_team_billing_meter_caches` before Management fetch
     (background FetchBilling honors TTL). **Honesty:** when prepaid $ is
     shown on `/limits`, note names process-cache lag + app last-good that can
     outlive TTL + that `grok limits` or opening `/limits` forces a fresh
     fetch. Joins:
     `.agents/joins/impl-prepaid-cache-ttl-polish-2026-08-02.md`,
     `.agents/joins/impl-tui-limits-force-refresh-2026-08-02.md`.
   - **TUI live postpaid shipped (with force-refresh wave):** `FetchBilling`
     live-calls Management postpaid preview into process cache (TTL honored
     unless explicit open/collect cleared). Modal rebuild still reads cache;
     explicit open clears first so postpaid is live like CLI.

   Meters stay distinct: personal SuperGrok **included weekly** ≠ SuperGrok
   **dollar extras** ≠ **console team prepaid** ≠ **console team postpaid
   OAuth/API class (Usage $)** ≠ second SuperGrok OAuth principal (Business
   SuperGrok session is not console team prepaid).

   **SuperGrok $ extras field map (dogfood 2026-08-01; do not invent $):**
   - SuperGrok Extra Usage Credits from session path only:
     `GET {cli-chat-proxy}/billing?format=credits` →
     `config.prepaidBalance.val` (USD cents). Product label: SuperGrok dollar
     extras. Live dogfood: personal **and** business OIDC both returned
     `prepaidBalance=10029` ($100.29) with `isUnifiedBillingUser=true` (one
     shared consumer Extra Usage Credits pool, not two stacks).
   - Also on that response (not SuperGrok $ balance): `creditUsagePercent`,
     `currentPeriod`, `onDemandCap`/`onDemandUsed` (0 when unified),
     `productUsage` (included % by product), `topUpMethod`. Auto-topup rule
     is a separate `GET …/auto-topup-rule` (amounts, not wallet total).
   - **Not** in GetGrokCreditsConfig: any larger SuperGrok wallet than
     `prepaidBalance`. If grok.com Settings → Usage shows a different Extra
     Usage Credits figure, that gap is **website-only** until xAI exposes
     another field we call — do not invent dollars.
   - **Console team prepaid** is a **different** meter:
     Management `GET …/billing/teams/{team_id}/prepaid/balance` (needs
     management key + `management_team_id`). Only balance field is
     `total.val` (USD cents string, often negative remaining). Product
     maps abs → cents → `$N` as **console team prepaid** only.
   - **Live dogfood 2026-08-02 (team 61fab250…):** prepaid wire
     `total.val="-34000"` → product **$340** (correct parse; not a
     double-divide bug). No second balance on that endpoint. Live
     postpaid invoice preview (Slice 3 M3 now product-wired for OAuth vs
     API class on limits; still **not** folded into prepaid `$N`):
     `defaultCredits` **$1500**; period spend-ish **~$207**; soft
     spending limit **$0**. Dashboard ~**$1317** is best read as
     **default credits remainder / composite** (e.g. $1500 − ~$183),
     **not** pure prepaid wallet and not a field product drops. Keep
     prepaid ledger distinct; optional later **separate** meter for
     defaultCredits if wanted. Joins:
     `/tmp/grok-join-deep-prepaid-340-vs-1317.md`,
     `/tmp/grok-join-live-prepaid-wire-capture.md`,
     `/tmp/grok-join-impl-limits-credits-observability.md`.
   - Product fix (same wave): dual `/limits` under unified pool **shares**
     observed Extra Usage Credits across personal/business rows (no more
     half "no data yet"); sibling poll **remembers** `prepaidBalance` per
     identity. Still not a sum of two pools when unified.

   **Highest-value next (re-rank 2026-08-03 after multiproc + Item 5 + process
   pins):** Soft honesty + bare-resolve dual-auth landmines + prepaid TTL +
   TUI force-refresh + F1b soft product close-out + default
   `auto_use_included_limits` + multiproc SuperGrok billing/Management shared
   cooldowns + durable included poll history + Item 5 spend series and team
   default-credits line are **shipped**. C4 **ticket evidence package** is on
   disk (human/xAI files the ticket; free SuperGrok period debit still **not**
   proven). What unblocks further product work:
   **rebuild and dogfood** the shipped limits stack on a live binary;
   **Item 2** free SuperGrok period debit ticket (server-side; no invent debit);
   **Item 3** live extras-after-full (C5 code exists; never live-proved at
   included ≥ 100%); optional Management process-cache polish only if dogfood
   still wants it; **Phase R** rate limits by API type (Imagine, voice, and
   BYOK are **not** intentional rate-limit skips). Plan **Revise** workflow is
   process-pinned (host + project AGENTS); re-open only if product CTAs still
   misbehave. One-click copy (§13) **shipped**. Do **not** invent C4 SuperGrok
   debit. Live prepaid parse (**$340**) is **done**; do not re-litigate "why
   not $1317" as a parse bug.

   Plans (limits-first living):
   [`.agents/plans/limits-first-ideal-2026-08-02.md`](.agents/plans/limits-first-ideal-2026-08-02.md),
   [`.agents/plans/limits-first-api-fix-section-2026-08-02.md`](.agents/plans/limits-first-api-fix-section-2026-08-02.md).
   Older dual-auth:
   [`.agents/plans/plan-auth-preferred-roles-failover.md`](.agents/plans/plan-auth-preferred-roles-failover.md),
   [`.agents/plans/plan-secure-key-failover.md`](.agents/plans/plan-secure-key-failover.md),
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

**Limits residual = two halves + limits-first (both halves intended; pin
2026-07-30; **Limits before credits** always):**
**Half A shipped** (SuperGrok session meters: dual principals, sibling poll,
`/limits` dual rows, footer honesty for included weekly + SuperGrok $ extras).
Not wrong-target waste; keep it. **Half B core prepaid shipped (2026-07-30):**
management key store, `management_team_id`, GET prepaid/balance, footer +
`/limits` console team prepaid labels (see §4). **Soft `/usage` console-live
honesty also shipped.** Honest **distinct** gaps when unknown:
`no management key` | `no management team id` | `loading team prepaid...` |
`team prepaid unavailable` | else `$N`.
**Limits-first product slices shipped 2026-08-02:** Slice 1 poll history /
flat honesty; Slice 3 M3 postpaid OAuth vs API class + C6 note; Slice 4
SuperGrok $ extras before console after included full (C5 code). Joins under
`.agents/joins/impl-slice{1,3,4}-*-2026-08-02.md`.
**Slice 2 dogfood + residual-edges wave (same day):** dogfood join
[`.agents/joins/slice2-dogfood-g4-2026-08-02.md`](.agents/joins/slice2-dogfood-g4-2026-08-02.md);
soft honesty polish
[`.agents/joins/impl-branch-2b-honesty-2026-08-02.md`](.agents/joins/impl-branch-2b-honesty-2026-08-02.md);
bare-resolve audit
[`.agents/joins/impl-bare-resolve-console-edge-audit-2026-08-02.md`](.agents/joins/impl-bare-resolve-console-edge-audit-2026-08-02.md);
live recheck
[`.agents/joins/live-limits-recheck-2026-08-02.md`](.agents/joins/live-limits-recheck-2026-08-02.md).
**Operator-edges residual wave (same day):** C4 ticket evidence package
[`.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md`](.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md);
prepaid TTL polish
[`.agents/joins/impl-prepaid-cache-ttl-polish-2026-08-02.md`](.agents/joins/impl-prepaid-cache-ttl-polish-2026-08-02.md);
F1b soft close-out
[`.agents/joins/impl-f1b-attribution-soft-2026-08-02.md`](.agents/joins/impl-f1b-attribution-soft-2026-08-02.md);
live multi-poll attempt still blocked
[`.agents/joins/live-multipoll-flat-note-2026-08-02.md`](.agents/joins/live-multipoll-flat-note-2026-08-02.md).
**C4 still FAIL** (server-side; honesty held, not fixed free SuperGrok period
debit; evidence package ready for human/xAI ticket). Morning flat 65% / Build
54% / $100.29; later log weak **65→66** (Build still 54); earlier multi-poll
auth-fail window. **C1/C3 pass** on dumps. C5 code-only not live-proved. Do
**not** invent SuperGrok debit.
**Shipped 2026-08-03 (same wave family):** new/empty home defaults prefer free
SuperGrok period allowance (`auto_use_included_limits=true`; explicit false
preserved); multiproc SuperGrok billing + Management shared rate-limit
cooldowns; durable multi-process included poll history; Item 5 spend series +
team default-credits line; TUI force-refresh (2026-08-02) + soft prepaid TTL +
F1b soft product honesty. Live prepaid dogfood **done** ($340 wire; ~$1317 ≠
prepaid).
**Still open:** rebuild and dogfood the shipped limits stack; **Item 2** free
SuperGrok period debit ticket (human/xAI; no invent debit); **Item 3** live
extras-after-full proof; optional Management process-cache polish; **Phase R**
rate limits by API type (**Imagine, voice, and BYOK are not intentional
rate-limit skips**; dual-auth public-host paths are a credential fact only).

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
Report: `/tmp/grok-join-impl-skills-discoverability-6a125de7.md`.

**Compaction honesty:** session `plan.md` is soft. Durable residual is this
file + short reports + `AGENTS.md` / `FORK.md`. Implement via main-thread (L1)
coordinator → subagents (L2) → specialists (L3 max); short reports on disk under
`.agents/reports/` (legacy notes may still live under `.agents/joins/`).

**What unblocks parallelization next:** Soft honesty + bare-resolve dual-auth
landmines + prepaid TTL + TUI force-refresh + F1b soft product honesty +
**new-install free-period-first default** + multiproc billing/Management
cooldowns + durable included poll history + Item 5 series/default-credits are
**done**. Remaining high-value items: **rebuild/dogfood**, **server-side free
SuperGrok period debit ticket (Item 2 / C4)**, **live extras-after-full
(Item 3)**, optional Management cache polish, and **Phase R rate limits by API
type** (Imagine / voice / BYOK included). Do **not** invent SuperGrok debit in
parallel "fixes."

| Rank | Work | Why |
|------|------|-----|
| 1 | ~~**Default prefer free SuperGrok period allowance for new installs**~~ **shipped 2026-08-03** | Empty/new config → `auto_use_included_limits=true`; explicit false preserved; doctor + user-guide. Report: `impl-item1-default-prefer-free-allowance-2026-08-03`. |
| 2 | ~~**TUI force-refresh parity with CLI**~~ **shipped 2026-08-02** | Explicit TUI `/limits` open force-busts Management caches like CLI `grok limits`. Report: `impl-tui-limits-force-refresh-2026-08-02`. |
| 3 | ~~**Multiproc SuperGrok billing + Management shared cooldowns**~~ **shipped 2026-08-03** | Flock JSON store on billing + Management HTTP. Report: `impl-shared-rate-limit-billing-management-2026-08-03`. |
| 4 | ~~**Durable multi-process included poll history**~~ **shipped 2026-08-03** | `$GROK_HOME/included_poll_history/` ring. Report: `impl-durable-included-poll-history-2026-08-03`. |
| 5 | ~~**Item 5 spend series + team default credits**~~ **shipped 2026-08-03** | Documented Management POST series; default credits own line (not prepaid `$N`). Report: `impl-item5-spend-series-default-credits-2026-08-03`. |
| 6 | **Rebuild and dogfood** the shipped limits stack on a live binary | Code joins are green; operator still needs a rebuild + live window to trust multiproc, durable history, series, and force-refresh together. |
| 7 | **Item 2 / server-side C4** free SuperGrok period debit ticket (evidence ready; human/xAI; no invent debit) | Product honesty held. Dogfood flat 65/54/$100.29; log weak 65→66; Build still 54. Ticket brief: `c4-supergrok-debit-evidence-package-2026-08-02`. Debit still **not** proven. |
| 8 | **Item 3 live extras-after-full** (C5 code exists; not live-proved at included ≥ 100%) | Needs a live window where free SuperGrok period allowance is full and SuperGrok dollar extras stay positive. |
| 9 | **Optional Management process-cache polish** only if dogfood still wants it | Soft; force-refresh and TTL already shipped. |
| 10 | **Phase R rate limits by API type** (Imagine / voice / BYOK **not** intentional skips) | Billing + Management multiproc cooldowns shipped; other API types still open. Cite public docs with accessed date. |
| 11 | ~~Soft prepaid cache TTL / force-refresh polish~~ **shipped** | Lag note + force-bust; shared TTL const. Report: `impl-prepaid-cache-ttl-polish-2026-08-02`. |
| 12 | ~~F1b attribution soft residual~~ **product honesty complete** | M3 + C6 + close-out green. Soft leftover: browser $547 vs M3 window only. Report: `impl-f1b-attribution-soft-2026-08-02`. |
| 13 | Plan freeform `plan.md` menus (process/skills) only if dogfood still jars | Product chrome green; **Revise** workflow process-pinned 2026-08-03. **Behind** operator ask. |

**Not "parked forever":** Half B core prepaid is **shipped**. M3 postpaid
OAuth/API class is **shipped**. Slice 1 flat-poll history (process, then
durable multi-process) is **shipped**. Slice 4 extras-before-console (C5 code)
is **shipped** (not live-proved at included ≥ 100%). Soft `/usage` console-live
honesty is **shipped**. Soft `"no $ meter yet"` is **retired**. Residual-edges
soft honesty polish + bare-resolve rank wire-up are **shipped**. Prepaid TTL +
TUI force-refresh are **shipped**. F1b soft product honesty is **complete**.
Default prefer free SuperGrok period allowance is **shipped**. Multiproc
billing/Management shared cooldowns are **shipped**. Item 5 spend series +
team default-credits line are **shipped**. C4 evidence package is **assembled**
(ticket is human/xAI; free SuperGrok period debit not closed). One-click copy
chrome (§13) is **shipped**. Main open gaps: **rebuild/dogfood**, **server-side
C4/2b free SuperGrok period debit** (no clean controlled debit; Build especially
flat), **live extras-after-full**, optional Management cache polish, and
**Phase R rate limits by API type** (public Imagine / voice / BYOK paths must
share rate-limit policy; dual-auth host choice is separate). Do **not** re-claim
management key unwired. Do **not** re-dismiss console meters as website-only.
Do **not** treat Half A SuperGrok work as discarded. Do **not** invent SuperGrok
included debit. Do **not** claim full Business Usage charts done. Do **not**
treat Imagine / voice / BYOK as intentional **rate-limit** skips.

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
2d. **Limits-first Slice 1 poll history / flat honesty (shipped):**
   `cargo test -p xai-grok-shell --lib included_poll_history`
   `cargo test -p xai-grok-pager --lib flat_poll`
   `cargo test -p xai-grok-shell --lib extensions::billing::`
2e. **Limits-first Slice 3 M3 postpaid (shipped):**
   `cargo test -p xai-grok-shell --lib xai_management`
   `cargo test -p xai-grok-pager --lib limits_cmd`
   `cargo test -p xai-grok-pager --lib limits_honesty`
   `cargo test -p xai-grok-pager --lib limits_snapshot`
2f. **Limits-first Slice 4 extras-before-console / C5 (shipped):**
   `cargo test -p xai-grok-shell --lib -- auto_order_keeps_supergrok auto_after_included_and_extras auto_with_included_headroom auto_order_omits_console auto_both_included_exhausted resolve_auto_after_included_exhausted resolve_enforced_auto_use_included_limits resolve_auto_both_supergrok_exhausted`
   `cargo test -p xai-grok-shell --lib -- allowance_exhaust_from_billing`
2h. **Residual-edges soft honesty polish (shipped):**
   `cargo test -p xai-grok-shell --lib -- flat_evidence remember_build included_poll_history remember_dollar`
   `cargo test -p xai-grok-pager --lib -- limits_honesty flat_poll format_surfaces format_dual_principal format_flat_poll usage_summary limits_cmd:: limits_snapshot::`
2i. **Bare resolve / console-edge rank wire-up (shipped):**
   `cargo test -p xai-grok-shell --lib -- subagent_override_auth_rank_flags_fail_closed resolve_model_override_config_missing_parent_supergrok_only resolve_model_override_api_key_pin resolve_model_override_agent_config_auto_use sampling_config_auto_use_omits_console sampling_config_api_key_pin resolve_model_to_sampling_config_auto_use`
2j. **Prepaid TTL / force-refresh polish (shipped):**
   `cargo test -p xai-grok-shell --lib -- console_team_billing_meter_cache management_meter_cache clear_console_team`
   `cargo test -p xai-grok-pager --lib -- limits_honesty prepaid lag force_refresh limits_snapshot`
2k. **F1b attribution soft honesty (product complete):**
   `cargo test -p xai-grok-pager --lib -- limits_honesty c6_team_usage usage_summary_supergrok_live limits_json_surfaces_postpaid limits_json_postpaid console_key_on_file_requests_supergrok format_console_live_skips format_console_section human_output_names_console`
   `cargo test -p xai-grok-shell --lib -- format_human_auto_use classify_postpaid fetch_postpaid_preview_hermetic`
2g. **Live dogfood (operator / rebuilt binary; not cargo):**
   Baseline + recheck + multi-poll attempt + C4 evidence package:
   [`.agents/joins/slice2-dogfood-g4-2026-08-02.md`](.agents/joins/slice2-dogfood-g4-2026-08-02.md)
   (C4 fail / branch 2b; flat 65% / Build 54% / $100.29);
   [`.agents/joins/live-limits-recheck-2026-08-02.md`](.agents/joins/live-limits-recheck-2026-08-02.md)
   (log weak 65→66, Build still 54; tip CLI sparse timeouts; C4 not closed;
   C5 not live);
   [`.agents/joins/live-multipoll-flat-note-2026-08-02.md`](.agents/joins/live-multipoll-flat-note-2026-08-02.md)
   (two spaced polls; auth fail; `flat_poll` absent; included 66% / extras
   $100.29 / prepaid $340 / `console.isLive=false`);
   [`.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md`](.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md)
   (ticket brief ready; debit not proven). Optional further recheck when
   **session billing is healthy** on a **long-lived** process:
   `grok-oss limits --json` (or installed `grok limits --json`); confirm
   `liveSampling`, included %, `grokBuildUsagePct` / Build product when present,
   SuperGrok extras cents, dynamic flat note (only meters observed flat), Build
   % on `/limits`/`/usage`, console `teamPostpaid*` when management key warm,
   `console.isLive=false` under headroom. Do **not** invent C4 pass without a
   controlled meter step.
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
   `cargo test -p xai-grok-pager --lib -- retry_chrome_soft_reconnect stream_resumed_without_prior_retry clip_retry_reason retrying_activity_label retrying_label_shows_timeout`
   `cargo test -p xai-grok-shell --lib -- stream_started_emits_retry_state_stream_resumed`
   `cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason retry_footer_backoff stream_headers_timeout_defaults`
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
