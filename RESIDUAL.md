# Open residual (human intent and unfinished honesty)

**D0 — open only.** Finished work lives in [`FORK.md`](FORK.md), process docs,
or code — not only here. Closed campaign history:
[`doc/dev/campaigns/interject-todos-closed-2026-07.md`](doc/dev/campaigns/interject-todos-closed-2026-07.md).

## Open

- **Running grok-oss sessions (shipped 2026-08-16).** Slash `/running` (alias `/windows`) and CLI `grok-oss running` / `grok-oss running --json` list live grok-oss TUI windows from `$GROK_HOME/active_sessions.json`. Identity is `(pid, session_id)`. Missing heartbeat is activity `unknown`. Title is the on-disk session summary. Default headless stays unlisted unless `GROK_TRACK_HEADLESS` is already set. Leader daemons stay on `grok-oss leader list`. `/rebuild` still signals each live grok-oss PID once (dedupe by PID). This is not Agent Dashboard. Lasting truth: [`FORK.md`](FORK.md) Product **Running grok-oss sessions**; user-guide `04-slash-commands`, `17-sessions`, `23-dashboard` (cite only). Reports: `.agents/reports/impl-running-registry.md`, `impl-running-heartbeat.md`, `impl-running-slash-cli.md`, `impl-running-rebuild.md`, `impl-running-docs.md`. Live TUI still needs install plus quit/reopen before `/running` appears in an already-open window. **Not leftover implement of this feature:** the plan-present idle cue, sparse UI, or SuperGrok Heavy ranking. Those stay their own Open items.

- **SuperGrok Heavy ranking leftover (open; not a product-code diagnose).** Live chrome now says SuperGrok dollar credits for the prepaid SuperGrok top-up meter (slice 5 shipped). Omit the word extras in user-facing copy, residual, reports, board titles, comments that humans read, and process law. SuperGrok is paid. Never call SuperGrok free. **SuperGrok Heavy ranking optional label is not implemented.** SuperGrok Heavy is a real tier, distinct from standard SuperGrok. Personal SuperGrok Heavy and Business/Team SuperGrok Heavy are separate weekly compute pools. They do not combine. Switching workspace switches which pool is drawn from. Standard Business seats are SuperGrok. Heavy is an explicit upgrade. xAI does not publish fixed numeric quotas. Remaining percent is in the product Usage view for that workspace. The operator has SuperGrok Heavy and does not see it used. Board `bug:supergrok-heavy-not-used` owns the diagnose. This pin does not diagnose product code. Law: `AGENTS.md` hard constraint 4. Report: `.agents/reports/pin-omit-extras.md`. An occupancy check on this host (2026-08-16) found two SuperGrok OIDC principals already stored, one personal and one Team/Business; synthesis: `.agents/reports/ask-te-second-supergrok-login-stored.md`.

- **1.0.3 restack inventory (updated 2026-08-13).** First chrome-only pass was too narrow. Three reports: SQL (`.agents/reports/fork-gaps-sql-features-2026-08-13.md`), config (`.agents/reports/fork-gaps-config-options-2026-08-13.md`), remaining seams (`.agents/reports/fork-gaps-remaining-seams-2026-08-13.md`). **Restored in source this wave:** dual-auth hop after included SuperGrok period limits are full (`.agents/reports/bug-dual-auth-spend-hop-restore-2026-08-13.md`); unread config fields plus `/settings` rows (`.agents/reports/bug-config-unread-restore-2026-08-13.md`); `/spend` ingest of `usage.jsonl` plus remote samples / reconciliation rows (`.agents/reports/bug-spend-ledger-restore-2026-08-13.md`); leftover plan chrome. Last-session and cancel-resume are not SQL. **Honest leftovers:** `sampling_identity` is unused; host `~/.grok/docs` extract stays stale until the next product launch; live TUI stays the old 1.0.3 binary until a successful rebuild/install. SuperGrok is paid; say **included SuperGrok period limits**. When the prepaid SuperGrok top-up meter is meant, say SuperGrok dollar credits.

- **Workspace fuzzy search reuses one matcher per root (2026-08-12, shipped).** Opening many workspace fuzzy searches without `close` no longer grows a new nucleo worker pair per `open`; `FuzzySearchManager` keeps one live search per cwd/root, and poll-only `get_results` does not reset the stale timer. Restack onto Grok Build 1.0.3 (2026-08-12) kept this contract in `xai-grok-workspace` plus `xai-fuzzy-file-search` `Nucleo::new(..., Some(2), 1)`.

- **Onto restack onto public Grok Build 1.0.3 (2026-08-12).** Product stack replayed onto `e5fd4816`. PR branch `onto-xai/b13fa526f511` joins `origin/main` `f17e84d8`. Shell `--lib` and pager `--lib` compile after the mop. Nucleo reuse-per-root tests are green. History-search lazy spawn tests are green. **Plan panel CTAs (2026-08-17):** Approve / Clarify / Revise / Exit as clickable footer buttons. Notes is gone. Letter `a` / `A` type. Shell `"questions"` for Clarify. Dual-auth hop after included SuperGrok period limits are full is restored in source (`sampling_config_for_model` fills the hop list; SuperGrok dollar credits keep SuperGrok primary). Live TUI stays the old 1.0.3 placeholder until a successful rebuild/install. Do not treat dogfood as done until install plus this PR is pushed.

- **Rust 1.97.1 + CI unit mop (2026-08-12, shipped, not open).**
  Project pin is `rust-toolchain.toml` / fenix **1.97.1**. Surmount keeps that
  pin even if an upstream export still lists 1.94.x (FORK packaging note).
  Unit clusters greened this wave: worktree/export hermetic git, team-managed
  dark seam, theme+hooks, shell stack/interject/usage/stream/external-auth.
  Package clippy mop for shell / pager / tools / update / workflow is green
  under `-D warnings`. **Still open (not this mop):** operator dogfood after
  install (first Open bullet); PTY grandchild-kill flake reliability
  (`close_pty_kills_a_background_grandchild`); a later second onto/join
  after this mop is finalized. Do not start a second onto from residual.

- **Dogfood / next-session gate (2026-08-09; open until install + dogfood done).**
  Tree work for the plan wave (A/B/C/E chrome, plan Revise, stale plan batch,
  OAuth 403 bad-credentials, rewind checkpoints, Ctrl+C rewind) is **in FORK**
  as shipped product. **Operator still needs** install + full quit of old TUIs
  + reopen `grok-oss` before treating chrome as live. Checklist:
  [`.agents/reports/d0-dogfood-checklist-2026-08-09.md`](.agents/reports/d0-dogfood-checklist-2026-08-09.md).
  Handoff: [`FORK.md`](FORK.md) § *Dogfood / next session handoff (2026-08-09)*.
  **Shipped in source (2026-08-16), not live until new binary:** mid-turn
  `/rebuild` now writes continue-interrupted-turn (`canceled_turn_resume.json`)
  before re-exec. Session load already applied that marker. Named test:
  `handle_rebuild_done_mid_turn_writes_cancel_resume_and_session_load_continues_the_turn`.
  Report: `.agents/reports/bug-rebuild-no-graceful-resume.md`.
  **Still not shipped:** auto-resume after an error-terminal turn with no
  marker (the older rebuild-auto-resume-after-error slice). Soft-stop
  **button** and mid-sample freeze-without-cancel are **not** shipped.
  Included SuperGrok period C4 stays server ticket (below §4).
  Post-dogfood process feature still open: thoughtful todos (next Open
  bullet). Operator dogfood after install stays open in this item. Live
  windows that already quit without re-exec stay gone until reopen.

- **Structured Rust edit format and lint (product shipped 2026-08-15).**
  After ACP `search_replace` / `apply_patch`, a `.rs` write means rustfmt
  that file and clippy-driver that file. Not `cargo clippy -p <crate>
  --lib`. The command-running tool refuses crate-wide cargo and does not
  start those commands. Honest `cargo test -p <crate> --lib <filter>`
  stays allowed. Kill switch: `GROK_SKIP_EDIT_VERIFY=1`. FORK subsection
  **File-level infer-from-path verify**. Named tests:
  `rust_edit_verify` (25) and `dangerous_cargo` (11). Reports:
  `.agents/reports/impl-edit-verify-89e0807b.md`,
  `.agents/reports/fork-file-level-edit-verify.md`. Effort-3 three-way
  review was cancelled by the operator (lookalike Review rows). Do not
  re-fan that review. Thoughtful todo tracking stays its own residual
  bullet below. Live snapshot:
  `.agents/reports/live-tasks-2026-08-15.md`.

- **ACP edit tools take a per-path write lock (shipped 2026-08-15).**
  `search_replace`, `apply_patch`, `write`, OpenCode `edit`, and
  `hashline_edit` (`GrokBuildHashline:hashline_edit`) acquire the path
  automatically as part of the tool call. Happy path is silent (no lock
  argument). A held path is a tool error that names the holder and the
  file. The tool does not write, wait, or show a human steal, skip, or
  wait menu. Agents resolve the conflict by talking to each other. Lock
  releases when the call finishes. File-level infer-from-path verify
  still runs under the same hold. OpenCode `edit` acquires after
  directory, same-string, and bulk-edit checks, and holds the lock
  through rustfmt and clippy-driver on the same `.rs` path.
  `hashline_edit` acquires on the joined path after
  `resolve_model_path` so it collides with the other tools on the same
  file. FORK subsection **ACP per-path write lock**. Named tests:
  module filter `per_path_write_lock` (original ACP fixtures plus
  `hashline_edit_refuses_when_another_agent_holds_the_path` and
  `hashline_edit_happy_path_does_not_mention_the_lock`); OpenCode edit
  module `opencode_edit_cannot_write_a_path_another_agent_already_holds`
  (not in the `per_path_write_lock` module). Reports:
  `.agents/reports/impl-acp-file-edit-lock.md`,
  `.agents/reports/impl-opencode-edit-lock.md`,
  `.agents/reports/impl-hashline-edit-lock.md`.

- **Tools improve tools (pinned 2026-08-15; process law, not a product
  slice).** Do not write disposable bash, Python, or one-off `curl` as
  agent glue. Improve the named product tools. Dual-pin: project
  `AGENTS.md` hard constraint 6; host `~/.grok/AGENTS.md` same heading;
  skill-rules rule 17. Report:
  `.agents/reports/pin-tools-improve-tools.md`.

- **L2 wait on a live L3 (shipped 2026-08-15).** Nested spawn is
  reparented to the L1 root for limits and stop. Query used only that
  root parent, so the L2 session got `not_found` while the parent
  still saw the live L3. Visibility now also matches the immediate
  spawner (`belongs_to_session`: root parent or `spawned_by_session`).
  A blocking query for an id the coordinator has not seen yet is
  parked for 250ms. When Spawn is processed, matching waiters attach
  to the live child with the caller's full block budget. If Spawn
  never arrives, the wait is `not_found` (unknown id stays under 2s).
  A foreign session still gets `None`, including a blocking query of
  an existing id. Attach uses the live child's session, not a later
  duplicate Spawn from the foreign session. A non-blocking snapshot of
  an unseen id stays `None`. Reparent-fail and parent-stopped spawn
  paths reject parked waits. Same-session duplicate-id spawn still
  attaches. Named tests:
  `spawner_can_wait_on_the_id_it_just_received_while_the_task_is_live`,
  `returned_spawn_id_is_waitable_before_coordinator_processes_spawn`,
  `foreign_blocking_query_of_a_live_id_stays_none_when_that_session_spawns_the_same_id`.
  Sibling fixtures still green:
  `blocking_query_of_unknown_id_returns_immediately`,
  `session_backend_cannot_query_or_cancel_foreign_child`. Reports:
  `.agents/reports/bug-l2-wait-l3-not-found.md`,
  `.agents/reports/impl-spawn-id-waitable.md`,
  `.agents/reports/fix-foreign-query-must-not-park.md`.

- **`/view-plan` while a live present is waiting (OPEN leftover).**
  Board: `ask:view-plan-while-live-present-missing-park`. Source idle-cue
  paint (`bug:plan-present-idle-written-cue`) shipped 2026-08-16: a fresh
  present first-paints **Plan ready. Side panel open** with five footer
  CTAs. Composer staying typeable is correct. Do not treat present as
  Approve. Soft present is now a real right-side pane (2026-08-16 seven
  slices). Leftover: if this window has no park but the shell is still
  waiting on `x.ai/exit_plan_mode`, should `/view-plan` answer that
  waiter, or may it open a local idle panel whose Approve does not
  complete the tool? Do not invent a third park. Map report:
  `.agents/reports/bug-plan-present-idle-written-cue.md`.

- **Wait holds composer (OPEN leftover; do not implement from closeout).**
  Board: `wait-holds-composer`. Long implement waits can still leave the
  composer feeling stuck. That is a product rewrite, not docs. Named in
  residual only. The 2026-08-16 seven slices did not ship it.

- **Plan present idle cue (shipped in source 2026-08-16; not leftover
  implement).** Board `bug:plan-present-idle-written-cue`. Named fixture
  `present_then_turn_finalize_without_park_still_paints_plan_ready_not_idle_click_cue`.
  Live TUI still needs install plus quit/reopen. Reports:
  `.agents/reports/impl-plan-present-idle-written-cue.md`,
  `.agents/reports/review-plan-present-idle-written-cue.md`.

- **Agent process: more thoughtful todo tracking (OPEN — plan thoughtfully later).**
  Operator (2026-08-09): session board / `todo_write` fib leaves and
  multi-track guards shipped a first cut, but the **operator experience of
  tracking work** still needs a deliberate plan (what the board is for, how
  residual vs session todos split, when agents over- or under-track, closeout
  honesty, less noise). **Do not invent** a second board system or mass-rewrite
  todos in this wave. Plan as its own design slice after dogfood priority work.
  Board: `feat:thoughtful-todo-tracking-process`. Shipped baseline remains in
  [`FORK.md`](FORK.md) (fib progress, clear finished, also-guard bind). Soft
  remainders under Open item 0 (structured todos) still apply.

- **Plan approval UI (product chrome shipped → FORK; agent freeform still soft):**
  soft-park **auto-opens** the plan side panel; footer mouse CTAs hit-tested;
  card re-reads live `plan.md`; L1 typing stays modal-free. `/view-plan` still
  reopens if dismissed. **Shipped 2026-08-10 (FORK P1–P3 + prior):** sticky
  `plan_decision_resolved` (no re-arm Approve after one decisive Approve/Quit
  until a new present); `exit_plan_mode` tool body present-only (not false
  “auto-approved”); always-approve ≠ plan panel Approve; **empty Enter never
  approves** (clickable Approve); present status **Plan ready.
  Side panel open**; after Revise/Clarify, **Revising plan...** / **Waiting for
  updated plan...** with no idle CTA re-arm until re-present (honest queue
  toast when channel closed); revise landing also pushes a human line, clears
  ghost composer draft, and shows real busy turn chrome (not barren
  Waiting+Enter:queue); composer caret empty half **text_primary** (not neon
  green letter ink). Lasting truth: [`FORK.md`](FORK.md). User-guide
  `19-plan-mode`, `03-keyboard-shortcuts`, `06-theming`,
  `22-permissions-and-safety`. Reports:
  [`.agents/reports/impl-p1-plan-decision-surface-2026-08-10.md`](.agents/reports/impl-p1-plan-decision-surface-2026-08-10.md),
  [`.agents/reports/impl-p2-revise-loop-chrome-2026-08-10.md`](.agents/reports/impl-p2-revise-loop-chrome-2026-08-10.md),
  [`.agents/reports/impl-p3-green-letter-caret-2026-08-10.md`](.agents/reports/impl-p3-green-letter-caret-2026-08-10.md),
  [`.agents/reports/impl-p4-docs-fork-survive-2026-08-10.md`](.agents/reports/impl-p4-docs-fork-survive-2026-08-10.md),
  [`.agents/reports/impl-revise-barren-wait-2026-08-10.md`](.agents/reports/impl-revise-barren-wait-2026-08-10.md).
  **Still soft:** agent-written `plan.md` can invent freeform "reply approve /
  options 1–5" (product chrome does not; process law = product CTAs only).
  **Still open leftover:** `ask:view-plan-while-live-present-missing-park`
  (do not invent a third park). Idle-cue first paint shipped in source
  2026-08-16. Soft present is a real right-side pane; click does not enter
  Commenting.

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
   **auto-opens** non-capturing plan **side panel** (toast/status **Plan ready.
   Side panel open**, not a `/view-plan` nudge). L1 stays modal-free (printable
   → composer). Approve/quit via **mouse footer CTAs**, side panel, status
   chip; empty Enter never approves; `/view-plan` reopens if dismissed. Force
   fullscreen: `plan_approval_park = "modal"`. **2026-08-10 also shipped
   (P1–P3):** present ≠ Approve (honest tool body + shell panel Approve vs
   no-client messages); sticky no multi-Approve after decide; always-approve is
   tool permissions only; revise/clarify in-flight status + no idle CTA re-arm;
   caret empty half `text_primary`. Lasting truth: [`FORK.md`](FORK.md).
   Design note (historical):
   [`doc/dev/research/plan-modal-softer-park-2026-07-26.md`](doc/dev/research/plan-modal-softer-park-2026-07-26.md).
   **Still soft:** agent-written `plan.md` freeform menus; toast may still
   *feel* modal to some operators. Do not invent a third park mode.
   **Still open leftover:** `ask:view-plan-while-live-present-missing-park`.
   **Shipped 2026-08-16 (seven slices, not leftover):** real right-side
   dock, click-row does not Commenting, screenshot paste plus Linux
   `Event::Paste` probe, Approve/Revise drain image chips.

2d. **Plan approval: real clickable CTAs + fresh plan.md (product chrome shipped
   2026-07-29; agent plan.md freeform still open)**
   **Shipped:** soft-park footer mouse CTAs (hit-tested; draft durable; Prompt
   focus paint); scrollback card not a fake button menu; empty placeholder not
   a key list; FileBacked panel + soft-park card re-read live `plan.md` (in-place
   card update, no second-card spam). Boards `bug:plan-cta-no-click-buttons`,
   `bug:plan-approval-stale-snapshot` product side green. Sticky multi-approve,
   false tool-body auto-approve, empty Enter no-op, revise-loop chrome, caret
   empty half: green 2026-08-10 (FORK P1–P3).
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
   SuperGrok **dollar credits** ≠ included SuperGrok period **limits** ≠ console
   team prepaid / console API **credits** ≠ second SuperGrok OAuth identity.
   When billing or limits come up, name which meter.
   **Vocabulary (pinned 2026-08-08; omit extras 2026-08-16):** say **limits**
   not bare "allowance". When the prepaid SuperGrok top-up meter is meant, say
   SuperGrok dollar credits. SuperGrok is paid; never call SuperGrok free. Full
   names when it matters: included SuperGrok period limits; SuperGrok dollar
   credits; console team prepaid / console API credits. Spend order: included
   SuperGrok period limits first, then SuperGrok dollar credits, then console.
   Never invent included SuperGrok period used % on the client. Project law:
   `AGENTS.md` hard constraint 4.

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

2i. **Multi-track / also guard (first cut shipped → FORK; soft remainders)**
   **Shipped (2026-08-07):** `meta.taskId` bind on session board items;
   `todo_write` merge rejects `in_progress` → `pending` while the bound
   subagent is still Running; complete/cancel always allowed; unbound demote
   still allowed; prompt + tool description teach bind-after-spawn. Report:
   [`.agents/reports/impl-ctrl-c-killall-resume-also-guard-2026-08-07.md`](.agents/reports/impl-ctrl-c-killall-resume-also-guard-2026-08-07.md).
   Lasting truth: [`FORK.md`](FORK.md).
   **Still soft:** auto-bind on every Task without agent meta; sticky toast
   specifically on new user message while multi-track live (title Agents +
   queue hold already exist); full todo↔agent ownership UI; reject demote for
   *all* live tracks without meta. Process dual-pin still applies.

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
   operator-edges residual wave same day: prepaid TTL polish + F1b soft close-out
   shipped; C4 paste-ready ticket assembled 2026-08-07 (operator must file;
   client levers exhausted; not agent soft-park); product client invent for
   limits-first exhausted**, not "wrong-target waste." SuperGrok work was
   correct and remains wanted. Do **not** invent SuperGrok included debit (C4)
   as proven: product honesty holds; server debit under load is still not a
   clean pass (prior flat 65/54; this period ~6% free period + rising team
   OAuth). Ticket path:
   [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md).
   Do **not** claim full Business Usage charts done; core prepaid + postpaid
   OAuth/API class meters are shipped.

   ### Half A — SuperGrok session billing meters (**shipped**, keep)
   Dual principals, sibling poll, `/limits` dual rows, footer credit-bar
   honesty for included weekly + SuperGrok $ extras. Useful; incomplete only
   relative to the **full** two-half ask. Detail under shipped bullets below.
   Do **not** discard or reframe Half A as pure waste.
   **Dual SuperGrok billing poll honesty (Option B, 2026-08-07):** process-local
   poll outcome per identity; CLI/JSON role + re-login fail notes;
   `pollSucceeded` / `includedSource` (live_poll | process_cache |
   shared_pool_fill); labeled unified fill; active free-period chrome not
   painted healthy from sibling-only when active poll AuthFailed; doctor dual
   poll health; rank prefers poll-OK SuperGrok (auth-failed demotes free-period
   cache headroom). Filters: `auth_failed_poll`, `dual_fill_provenance`,
   `order_live_prefers_poll_ok`, `format_human_dual_poll` in
   [`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md) §2c.
   Report: `.agents/reports/impl-dual-supergrok-billing-honesty-2026-08-07.md`.
   **Also shipped (2026-08-07 hourly):** demote sibling from automatic billing
   poll list after **3** consecutive auth-class fails (`SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD`);
   Ok resets streak; network fails do not bump; **never** auto-deletes
   `auth.json` secrets. Filter:
   `sibling_poll_skips_after_n_consecutive_auth_fails_without_secret_delete`.
   Report: `.agents/reports/hourly-residual-2026-08-07-2043.md`.
   **Also shipped (2026-08-07 hourly):** multi-slot OIDC refresh **before**
   sibling billing poll when JWT is past early-invalidation buffer and still
   has refresh_token + issuer + client_id; persists refreshed multi-slot only
   (does not clobber active base of another principal; never deletes secrets).
   Filters: `session_needs_oidc_refresh`, `ensure_fresh_refreshes_expired`,
   `find_and_persist_refreshed`. Report:
   `.agents/reports/hourly-residual-2026-08-07-2143.md`.
   Dual SuperGrok poll soft edges (N-fail demote + multi-slot OIDC refresh)
   are **closed** for product residual.

   ### Half B — console team Usage class meters (**core prepaid + M3 postpaid
   + SuperGrok-live team visibility shipped; license charts non-goal; series
   charts UI still optional**)
   TUI picture of team prepaid / postpaid OAuth vs API class / optional token
   spend charts (console product, Team Surmount), via xAI **Management API** +
   `team_id`. **Naming:** Half B "Business Usage class" = team **API** prepaid /
   postpaid / USD series — **not** Platforms → Grok Business **licenses**
   (messages / conversations). License page zeros are expected for CLI SuperGrok
   dogfood; product does not drive that page. **Do not invent scrape of
   console.x.ai HTML** or fake license endpoints.

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
   - **Included SuperGrok period before credits chrome (shipped 2026-08-07):** sticky
     exhaust memo must not paint `console · $N` while live included SuperGrok period
     still has headroom (`status_sampling_identity_for_compact_meter` + Design A).
     `limits --json` / human **Active:** `activeDriver` names included SuperGrok
     period limits | SuperGrok extras | console key. Settlement dual-bill (C6 + flat)
     free period note) stays distinct from SuperGrok extras. Client TE vertical
     complete; **C4 server free-period debit** still open below.
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
     `Balance: $N`; honest **distinct** gaps when unknown: `no management key`
     | `no management team id` | `loading team prepaid...` |
     `team prepaid unavailable` (soft `no $ meter yet` and mushy
     `no management key/team id` retired)
   - User-guide: `02-authentication` + `04-slash-commands` `/limits`
   Joins: `/tmp/grok-join-impl-mgmt-key-team-fetch-2026-07-30.md`,
   `/tmp/grok-join-impl-console-meter-tui-2026-07-30.md`,
   `/tmp/grok-join-impl-no-dollar-meter-real-0c6a7911.md`.

   **Also shipped (soft `/usage` console-live honesty — 2026-07-30):**
   When console is the live sampling principal, non-silent `/usage` names
   **console team prepaid** (or honest gap family above) and does **not** sell
   SuperGrok session billing / SuperGrok $ extras as live console spend.
   Report: `/tmp/grok-join-impl-usage-console-honesty-0c6a7911.md`.

   **Also shipped (SuperGrok-live team Management visibility — 2026-08-04):**
   Footer and `/usage` surface team prepaid $ (or loading/unavailable gap)
   while SuperGrok session is live; `/limits` Console API Balance stays when
   `console.isLive=false`; license-page honesty note (licenses messages/
   conversations ≠ SuperGrok / team Management). FetchBilling already refreshed
   prepaid+postpaid regardless of console live. **Non-goal:** Grok Business
   license charts non-zero. Report:
   [`.agents/reports/impl-supergrok-live-team-usage-2026-08-04.md`](.agents/reports/impl-supergrok-live-team-usage-2026-08-04.md).
   OAuth-after-period-reset dogfood:
   [`.agents/reports/plan-oauth-after-period-reset-2026-08-04.md`](.agents/reports/plan-oauth-after-period-reset-2026-08-04.md).

   **Also shipped (license zeros honesty + Grok Build class prominence —
   2026-08-07):** Doctor + user-guide + sharper `/limits` note: Platforms →
   Grok Business **licenses** Usage zeros are **expected** for CLI SuperGrok
   (not dogfood proof). Real burn = team Usage / Grok Build class $ (browser
   team `.../usage` + Management postpaid OAuth / series) and SuperGrok meters.
   P1: `/limits` Console puts **Team postpaid OAuth / Grok Build class** near
   top when known; SuperGrok-live footer chip `team Grok Build class: $N` when
   postpaid OAuth class is in Management process cache (separate from team
   prepaid and included SuperGrok period %). Design A compact free-period `%`
   unchanged. Dual SuperGrok poll honesty stays closed for this problem. Report:
   [`.agents/reports/impl-grok-business-license-zeros-vs-team-usage-2026-08-07.md`](.agents/reports/impl-grok-business-license-zeros-vs-team-usage-2026-08-07.md).

   **Also shipped (P2 usage series on FetchBilling / `/limits` warm path —
   2026-08-07):** Management usage series (POST analytics; OAuth / Grok Build
   class + API class + top descriptions) uses the same practical path as other
   Management meters: background `FetchBilling`, TUI `/limits` open (force
   clear then silent refresh), and CLI `grok limits`. Process cache shares the
   60s soft TTL with prepaid/postpaid; explicit open/collect busts series with
   the other billing meters (no unbounded spam). `/limits` rebuild attaches
   warm series into the Console block when known. Meters stay distinct (series
   USD ≠ team prepaid ≠ included SuperGrok period %). Full browser-style series
   **charts** UI still not shipped (text totals only). Report:
   [`.agents/reports/impl-p2-usage-series-fetch-billing-2026-08-07.md`](.agents/reports/impl-p2-usage-series-fetch-billing-2026-08-07.md).

   **Also shipped (Token Economy full product — 2026-08-03):** implement-effort
   policy under economic mode (ceiling 3 / desired 2 / all implement entry
   paths / clamp+toast); included SuperGrok period linear-burn pacing on
   credit/status + `/limits` + `/usage`; double-entry local vs Management on
   `/spend` and `/limits` section; durable **`$GROK_HOME/grok_oss.db`**. Report:
   [`.agents/joins/impl-token-economy-full-2026-08-03.md`](.agents/joins/impl-token-economy-full-2026-08-03.md).
   Lasting bullet: [`FORK.md`](FORK.md). **Operator dogfood only** (rebuild +
   management key for remote book) — not open code residual.
   **Token Economy further options plan is parked (2026-08-04), not cancelled**
   (resume later; do not implement TE pillars in the SuperGrok-live team usage
   exclusive priority).

   **Still open (limits-first + Half B remaining — do not invent included SuperGrok
   period debit on the client):**
   - **Settlement pay-path tracking gap (honesty + chrome labels shipped
     2026-08-09; machine payingMeter still soft):** Operator dogfood
     (console.x.ai team Billing **Credits ~$343**, auto top-up off, team
     `61fab250-…`) while product chrome said included SuperGrok period active
     (`activeDriver=supergrok_free_period`, `liveSampling=supergrok_session`,
     `console.isLive=false`, included SuperGrok period ~9%, SuperGrok dollar credits
     ~$100.29, team prepaid ~$340, team postpaid OAuth class ~$1163,
     `flatPollUnprovenDebit=true`). **Gap in plain English:** `activeDriver` /
     **Active:** is client **spend-order intent chrome**, not "who is paying."
     SuperGrok session traffic can still settle on **team postpaid OAuth /
     Grok Build class** and can change **console team prepaid remaining**
     (team Billing Credits) without included SuperGrok period used % moving and
     without the console API key being live. **Shipped honesty + chrome:**
     human Console line **Team prepaid remaining**; note
     `NOTE_ACTIVE_DRIVER_IS_INTENT_NOT_SETTLEMENT`; doctor dogfood block; Work C
     status compact `included SuperGrok period limits · N%` (not bare `intent ·`); SuperGrok-live
     footer: while included SuperGrok period has room, **no** long team prepaid /
     Grok Build class line (quiet footer; team wallets on `/limits`); after free
     SuperGrok period is full, optional secondary
     `not the active spend path: team prepaid remaining $N · Grok Build class $M`
     (cold: `not the active spend path: loading team prepaid...`). AuthMeta
     `usage_visible=false` still gates footer while included SuperGrok period compact paints.
     Prior `Team settlement:` misread as active pay; prior always-on
     `not the active spend path:` while included SuperGrok period had room was messy
     (fix 2026-08-09).
     Reports:
     [`.agents/reports/impl-team-settlement-chrome-vs-limits-2026-08-09.md`](.agents/reports/impl-team-settlement-chrome-vs-limits-2026-08-09.md),
     [`.agents/reports/impl-settlement-pay-path-tracking-gap-2026-08-09.md`](.agents/reports/impl-settlement-pay-path-tracking-gap-2026-08-09.md),
     [`.agents/reports/impl-work-c-meters-chrome-2026-08-09.md`](.agents/reports/impl-work-c-meters-chrome-2026-08-09.md),
     [`.agents/reports/impl-status-chrome-messy-team-prepaid-2026-08-09.md`](.agents/reports/impl-status-chrome-messy-team-prepaid-2026-08-09.md).
     **Still soft / not shipped:** machine `payingMeter` field that claims
     which wallet actually debited last request (needs settlement deltas, not
     invented included SuperGrok period %); prepaid remaining **delta** history as
     first-class burn proof; compact status primary chip that prefers settlement
     over included SuperGrok period % (would fight Design A free-period-first chrome
     on purpose). Do **not** invent included SuperGrok period debit on the client.
   - **Included SuperGrok period stuck ~6% — client SessionToken principal fix
     shipped 2026-08-08; dogfood next (do not assume server-only):**
     Operator correction: included SuperGrok period **used to work**; treat as
     **our client regression** until product path is proven. **Client bug
     found:** dual SuperGrok free-period rank could pick personal primary while
     sticky AuthManager **Team base** still drove SessionToken
     `cli-chat-proxy` bearer (team OAuth / Grok Build settlement climbs; free
     SuperGrok period % flat). **Fix:** align AuthManager base + wire bearer to
     included SuperGrok period ranked primary before SessionToken reconstruct and
     prepare_sampling (`align_to_ranked_free_period_primary`). Report:
     [`.agents/reports/impl-free-period-client-path-bug-2026-08-08.md`](.agents/reports/impl-free-period-client-path-bug-2026-08-08.md).
     Installed: `just install` → `grok-oss 0.2.111`. **Open dogfood:** free
     SuperGrok period used % must rise under SuperGrok session after install
     (or SuperGrok dollar credits after free period is full). **Do not** file
     xAI ticket as the only answer until dogfood after this fix fails with
     path traces showing ranked JWT is wire-active. Prior multipoll/C4 packages
     stay as attachments if server debit is still flat after path proof:
     [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md),
     multipoll addenda, flat-poll honesty / optional hard block
     (`allow_spend_when_free_period_debit_unproven`). Client invent of free
     SuperGrok period debit remains **banned**. **C5** Slice 4 still
     code-only (not live-proved at ≥ 100%).
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
     **Phase R rate limits by API type (shipped 2026-08-03; verified green
     2026-08-07):** Imagine image/edit, Imagine video, Voice STT, Responses
     (`web_search`), chat/BYOK (sampler host+fingerprint), billing, Management,
     and GitHub update all wait/observe the flock store with type-appropriate
     keys. Lasting truth: [`FORK.md`](FORK.md) § Multi-session rate limits.
     Join: [`.agents/joins/impl-phase-r-rate-limits-by-api-type-2026-08-03.md`](.agents/joins/impl-phase-r-rate-limits-by-api-type-2026-08-03.md).
     Prior audit (credential path ≠ rate-limit skip):
     `.agents/joins/console-bypass-paths-code-audit-2026-08-02.md`.
   - **Multiproc SuperGrok billing + Management shared cooldowns (shipped
     2026-08-03):** SuperGrok session billing and Management API HTTP paths
     wait on / observe the flock JSON shared rate-limit store under
     `$GROK_HOME/rate_limits/`. Report:
     [`.agents/joins/impl-shared-rate-limit-billing-management-2026-08-03.md`](.agents/joins/impl-shared-rate-limit-billing-management-2026-08-03.md).
     Phase R class table (Imagine / voice / BYOK / inference) is **shipped**
     (see bullet above + FORK).
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
   disk (human/xAI files the ticket; included SuperGrok period debit still **not**
   proven). What unblocks further product work:
   **rebuild and dogfood** the shipped limits stack on a live binary;
   **Item 2** included SuperGrok period debit ticket (server-side; no invent debit);
   **Item 3** live extras-after-full (C5 code exists; never live-proved at
   included ≥ 100%); optional Management process-cache polish only if dogfood
   still wants it. **Phase R** rate limits by API type is **shipped** (Imagine
   / voice / BYOK / chat share flock cooldowns with class keys; dual-auth public
   host paths remain a credential fact only). Plan **Revise** workflow is
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
**Operator-edges residual wave (same day):** prior C4 evidence package
[`.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md`](.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md);
prepaid TTL polish
[`.agents/joins/impl-prepaid-cache-ttl-polish-2026-08-02.md`](.agents/joins/impl-prepaid-cache-ttl-polish-2026-08-02.md);
F1b soft close-out
[`.agents/joins/impl-f1b-attribution-soft-2026-08-02.md`](.agents/joins/impl-f1b-attribution-soft-2026-08-02.md);
earlier multi-poll attempt (auth-fail window)
[`.agents/joins/live-multipoll-flat-note-2026-08-02.md`](.agents/joins/live-multipoll-flat-note-2026-08-02.md).
**C4 (2026-08-07 hard address):** paste-ready ticket
[`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md)
is the operator deliverable. Server debit still unproven (prior flat 65/54;
this period ~6% free period + climbing team OAuth). Client levers exhausted;
not agent soft-park. **C1/C3 pass**. C5 code-only not live-proved. Do **not**
invent SuperGrok debit.
**Shipped 2026-08-03 (same wave family):** new/empty home defaults prefer free
SuperGrok period allowance (`auto_use_included_limits=true`; explicit false
preserved); multiproc SuperGrok billing + Management shared rate-limit
cooldowns; durable multi-process included poll history; Item 5 spend series +
team default-credits line; TUI force-refresh (2026-08-02) + soft prepaid TTL +
F1b soft product honesty. Live prepaid dogfood **done** ($340 wire; ~$1317 ≠
prepaid).
**Still open:** rebuild and dogfood the shipped limits stack; **Item 2**
operator files C4 paste-ready ticket (no invent debit); **Item 3** live
extras-after-full proof; optional Management process-cache polish. **Phase R**
rate limits by API type is **shipped** (Imagine / voice / BYOK / chat; FORK §
Multi-session rate limits; dual-auth public-host paths stay a credential fact).

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
**done**. Remaining high-value items: **dogfood included SuperGrok period after
SessionToken rank-align fix** (2026-08-08 report
`impl-free-period-client-path-bug-2026-08-08`), **live extras-after-full (Item 3)**,
optional Management cache polish. Server C4 ticket package stays **secondary**
until dogfood disproves the client principal path. **Phase R** is **shipped**
(Imagine / voice / BYOK included). Do **not** invent SuperGrok debit in
parallel "fixes."

| Rank | Work | Why |
|------|------|-----|
| 1 | ~~**Default prefer included SuperGrok period allowance for new installs**~~ **shipped 2026-08-03** | Empty/new config → `auto_use_included_limits=true`; explicit false preserved; doctor + user-guide. Report: `impl-item1-default-prefer-free-allowance-2026-08-03`. |
| 2 | ~~**TUI force-refresh parity with CLI**~~ **shipped 2026-08-02** | Explicit TUI `/limits` open force-busts Management caches like CLI `grok limits`. Report: `impl-tui-limits-force-refresh-2026-08-02`. |
| 3 | ~~**Multiproc SuperGrok billing + Management shared cooldowns**~~ **shipped 2026-08-03** | Flock JSON store on billing + Management HTTP. Report: `impl-shared-rate-limit-billing-management-2026-08-03`. |
| 4 | ~~**Durable multi-process included poll history**~~ **shipped 2026-08-03** | `$GROK_HOME/included_poll_history/` ring. Report: `impl-durable-included-poll-history-2026-08-03`. |
| 5 | ~~**Item 5 spend series + team default credits**~~ **shipped 2026-08-03** | Documented Management POST series; default credits own line (not prepaid `$N`). Report: `impl-item5-spend-series-default-credits-2026-08-03`. |
| 6 | **Dogfood included SuperGrok period after SessionToken rank-align** (`just install` done 2026-08-08) | Client fix: SessionToken bearer follows included SuperGrok period ranked primary, not sticky Team base. Report: `impl-free-period-client-path-bug-2026-08-08`. Expect included SuperGrok period used % to rise under SuperGrok session (or SuperGrok dollar credits after free period full). |
| 7 | **Server C4 included SuperGrok period debit ticket — only if dogfood after rank-align still flat** | Prior packages: `.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`. Do **not** treat as first answer; client principal bug shipped first. |
| 8 | **Item 3 live extras-after-full** (C5 code exists; not live-proved at included ≥ 100%) | Needs a live window where included SuperGrok period allowance is full and SuperGrok dollar extras stay positive. |
| 9 | **Optional Management process-cache polish** only if dogfood still wants it | Soft; force-refresh and TTL already shipped. |
| 10 | ~~**Phase R rate limits by API type**~~ **shipped 2026-08-03; verified 2026-08-07** | Imagine / video / voice / responses / chat+BYOK / billing / Management / GitHub. Join: `impl-phase-r-rate-limits-by-api-type-2026-08-03`. FORK § Multi-session rate limits. |
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
Default prefer included SuperGrok period allowance is **shipped**. Multiproc
billing/Management shared cooldowns are **shipped**. Item 5 spend series +
team default-credits line are **shipped**. **Phase R rate limits by API type**
is **shipped** (Imagine / voice / BYOK share policy; host choice ≠ skip). Free
SuperGrok period SessionToken rank-align **shipped 2026-08-08** (client
principal path; dogfood next). C4 paste-ready package remains **secondary** if
dogfood still flat after path proof. One-click copy chrome (§13) is
**shipped**. Main open gaps: **dogfood included SuperGrok period after rank-align**,
**live extras-after-full**, optional Management cache polish, multi-track
also-guard product. Dual SuperGrok poll soft edges (N-fail demote +
multi-slot OIDC refresh before sibling poll) are **shipped**. Do **not** re-claim
management key unwired. Do **not** re-dismiss console meters as website-only.
Do **not** treat Half A SuperGrok work as discarded. Do **not** invent SuperGrok
included debit. Do **not** claim full Business Usage charts done.

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
   `cargo test -p xai-grok-pager --lib -- format_dual_principals live_console_omits extra_principals_hook show_limits format_supergrok_session footer_names_live_principal limits_json_lists_two_supergrok_principals_when_both_slots_exist limits_json_honest_single_supergrok_session_cannot_see_team_plan`
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
   (prior ticket brief); paste-ready merge + multipoll 2026-08-07…08:
   [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md)
   (operator files once; debit not proven by client). Optional further recheck
   when **session billing is healthy** on a **long-lived** process:
   `grok-oss limits --json` (or installed `grok limits --json`); confirm
   `liveSampling`, included %, `grokBuildUsagePct` / Build product when present,
   SuperGrok extras cents, `flatPollUnprovenDebit` (after rebuild of 2026-08-07
   dense-window fix), dynamic flat note (only meters observed flat), Build % on
   `/limits`/`/usage`, console `teamPostpaid*` when management key warm,
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
