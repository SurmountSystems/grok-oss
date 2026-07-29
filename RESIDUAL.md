# Open residual (human intent and unfinished honesty)

**D0 — open only.** Finished work lives in [`FORK.md`](FORK.md), process docs,
or code — not only here. Closed campaign history:
[`doc/dev/campaigns/interject-todos-closed-2026-07.md`](doc/dev/campaigns/interject-todos-closed-2026-07.md).

## Open

- **Plan approval UI dead after soft-park (2026-07-27) — primary fixed:** root
  cause was soft-park CTA keys gated on `active_pane != Scrollback`, so reading
  the parked plan card killed Enter/a/A/?/s/q while the legend still showed.
  Fix: always route soft-park keys; focus Prompt on park; Preview footer lists
  CTAs. Join:
  `join-plan-ctas-dead.md`. Board `bug:plan-approval-ctas-dead` green.
  **Still soft:** card legend is text (not hit-tested buttons); empty prompt
  still required for soft-park keys (draft preserved). Panel footer buttons
  already worked via `/view-plan` / status click. Related process-only:
  `bug:exit-plan-mode-false-approve` (soft-park ≠ approve; tool "start coding"
  only after real CTA) — host pin `~/.grok/AGENTS.md` § Plan approval item 7.

0. **Structured todos: fib leaves + progress + no casual reset (shipped product)**
   **Shipped:** first-class optional `size` (1|2 only; reject 0/3/5/8…);
   `meta.size` fallback normalized into field; leaf-only
   `compute_leaf_progress` (points mode when any leaf sized; else legacy
   counts); reject size on parents with children; tool result includes
   `progress` + optional `merge:false` archive warning; status-bar badge
   shows `N/M pts` in points mode; `prompt.md` Planning + tool description
   teach merge-only + fib leaves. **Process dual-pin** already on host
   `_SKILL_RULES` / product AGENTS L1. **Still soft:** no hard ban on
   inventing bare ids; phase vs work tree is agent structure (not enforced
   hierarchy product). Plan:
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

2. **Plan approval soft park (option A shipped; B/C/D parked)**
   **Shipped:** when `exit_plan_mode` parks, durable approval + status chrome +
   toast (“Plan parked — press /view-plan or click status to review”) without
   auto-opening the line-viewer modal; modal still on demand via `/view-plan`,
   status click, or `ShowPlan` / `reopen_plan_approval`. Four CTAs, clarify RO,
   park/abandon durable. Track `feat:plan-modal-softer-park`. Design note:
   [`doc/dev/research/plan-modal-softer-park-2026-07-26.md`](doc/dev/research/plan-modal-softer-park-2026-07-26.md).
   **Parked (do not invent):** options B/C/D (side panel / inline card / config
   modal-vs-soft) and full non-modal redesign — only if dogfood shows option A
   toast still jars; A is not broken.

2d. **Plan approval: real clickable CTAs + fresh plan.md (OPEN — dogfood 2026-07-27)**
   Operator still sees keyboard-only footer text
   (`Enter/a approve · A · ? · s · q · /view-plan`) with **nowhere to click**,
   and the park/approval card can show a **stale title/body** (old usage-meter
   plan) while session `plan.md` on disk was rewritten (billing plan). Product
   law: **mouse/click primary** — real buttons (Approve, Approve with notes,
   Clarify, Revise, Quit); accelerators may mirror; never treat accelerator
   text as the UI. File-backed preview must **re-read `plan.md` on open**.
   Prior hit-area work claimed shipped (`bug:plan-cta-buttons`) — dogfood says
   incomplete or wrong surface. Session board: `bug:plan-cta-no-click-buttons`,
   `bug:plan-approval-stale-snapshot`. **Defer implement** until billing/limits
   (wrong credit pool / Business) is green — track only; do not burn tokens on
   this while personal SuperGrok extras are the live spend path.

2e. **TUI self-screenshot (parked feature — dogfood ask 2026-07-27)**
   Build can capture its own screen for agent debugging / dogfood (hard to
   screenshot retries by hand). Board: `feat:tui-self-screenshot`. **Park**
   until billing/limits dogfood-green; do not implement ahead of residual §4.

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
   remaining personal SuperGrok money; then plan mode or a dedicated plan
   pass. Also pin: parent must not keep multi-file doc edits in main chat.

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

3. **UI: hide header + DOGE (shipped H1+H2 + pure palette + polish)**
   **Shipped:** `[ui] hide_header` (default false) zeros top agent status bar,
   **welcome location top bar**, and **dashboard location header**; settings
   Appearance + live apply. Theme **`doge`** only (display “DOGE”; no
   `ecma-doge` / `rgbcmykw` / `ansi-8` aliases): pure `#000000`/`#FFFFFF` + 8
   primaries exactly; `requires_truecolor: false`; hard-threshold quantise
   util + tests; **context-bar solid DOGE steps** (no mid-gray lerp);
   **pure-primary `doge.tmTheme`** for syntax. OLED-friendly motivation only —
   no power claims. Project note (not an ECMA standard):
   [`doc/dev/specs/doge-pure-8-colour-2026-07-26.md`](doc/dev/specs/doge-pure-8-colour-2026-07-26.md).
   **Still open polish:** none on this slice (Wave 2 DOGE polish closed).

4. **OAuth SuperGrok ↔ console API key failover (rate-limit switch + durable memo shipped)**
   **Shipped:** first-party resolve merge (session primary + console key
   failover; `preferred_method=api_key` reverses, including aux/web-search);
   identity rotate on **credit / SuperGrok Heavy usage-limit** and **plain HTTP
   429**, and **also switch the API host** (cli-chat-proxy ↔ `api.x.ai`, proxy
   header strip/restore, bearer stash/reinstall); **credit/allowance**
   exhausted-fingerprint memo (1h TTL; process cache + durable
   `$GROK_HOME/exhausted_credits/`; preemptive skip survives restart;
   **console-key** success clears that fingerprint; **session** success does
   **not** clear — extras-paid SuperGrok 200s must not put SuperGrok back) +
   status chrome / toast (“… (out of allowance)” vs “… (rate limited)”,
   fingerprints only — no raw keys); when billing included `usage_pct ≥ 100%`
   + dual-auth, mark SuperGrok used up and prefer console key **before** the
   next request (no 402 so paid extras do not burn; clear on period reset /
   usage drop); rate-limit switch observes temporary shared `grok-rate-limit`
   cooldown for the left identity (not the credit memo, so primary can return
   when cool); kill-switch clears console failover + host-switch metadata; xAI
   console keys in keyring `grok-build` + `provider_credentials.json` (env wins);
   login TTY progress during keyring store. User-guide `02-authentication` +
   `11-custom-models`.
   **Also shipped (polish):** AuthManager **live re-bind without prior stash**
   (`session_bearer_resolver` durable; hop-to-session prefers stash then live
   re-bind; next turn re-resolves via `reconstruct_full_config`); **multi-add**
   console keys (`add_console_api_key` comma-list store; `grok login --api-key`
   multi-add; `grok login --list-api-keys` fingerprints only — never raw keys).
   **Still open (dogfood 2026-07-27) — deferred rigorous plan, not drive-by:**

   Operator clarification (same day; durable):
   - **Want included SuperGrok limits first** (personal weekly/monthly headroom),
     **not** prepaid SuperGrok dollar extras and **not** console API $ as the
     silent primary while limits remain. Current dogfood pin
     `preferred_method = "api_key"` burns console $ first (config, not hop bug).
   - **Not** asking for `preferred_method = "personal" | "business"`. Method pin
     stays **login session vs console API key**. **Role** (personal SuperGrok /
     Business SuperGrok / console key, etc.) should inform **failover order**
     and labeling, not replace method names.
   - **Naming (`feat:auth-preferred-aliases-roles`):** keep `api_key`; add serde
     aliases `console_api_key`, `api`, `key`. Prefer config/UI name **`oauth`**
     for the session method (aliases `oauth_token`; keep `oidc` as accepted
     alias). Plain language in doctor/status: "SuperGrok login" / "console API
     key", not bare `oidc`.
   - **Role-aware failover + multi SuperGrok store** (`feat:failover-any-live-limits`,
     slice `feat:second-supergrok-business`): plan later with full TDD matrix
     (defaults, hop combinations, meter per live identity). Do **not** invent
     ship shape in residual beyond: prefer free/included headroom before extras
     and console $; hop when hard-limited; meter = billed account.
   - **Meter honesty (`bug:credits-meter-wrong-pool`):** still open (console
     live shows `"no $ meter yet"`; SuperGrok extras meter can lie after hop).

   Meters stay distinct: personal SuperGrok **included limits** ≠ SuperGrok
   **dollar extras** ≠ **console API spend** ≠ Business SuperGrok **included/
   Heavy** (separate principal when multi-session exists).

   **Deferred plan (branch + session, survive compaction):**
   [`.agents/plans/plan-auth-preferred-roles-failover.md`](.agents/plans/plan-auth-preferred-roles-failover.md)
   (aliases + role/failover/TDD outline; implement only after explicit approve).
   Older: [`.agents/plans/plan-secure-key-failover.md`](.agents/plans/plan-secure-key-failover.md),
   [`.agents/plans/plan-rate-limit-failover.md`](.agents/plans/plan-rate-limit-failover.md).
   Joins: `join-dual-auth-audit.md`, `join-hop-wiring.md`,
   `join-failover-meter-intent.md`.

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

## Highest-value next (product residual only)

**Wave 0 / 0b / Wave 1 core shipped** (ASCII S0–S4, plan selection P1–P4, T4
`json_to_toon`, session_reader Codex+Cursor SQLite, plan soft-park A,
implement-memory + plan_validate intercepts). Soft polish below is **parked
unless dogfood or operator re-ranks** — not next-wave must-do.

**Parked / skip (honesty close-out 2026-07-26):** plan soft-park B/C/D (A fine);
`send_now_*` → interject rename (cosmetic only); first-class memory/plan-validate
tool registration (intercept path enough; no dogfood demand).

**What unblocks parallelization next:** real ranked rows are sparse and
disjoint from parked polish. Fan out only on non-overlapping paths. Do not
parallel two writers on the same dual-auth hop files.

| Rank | Work | Why |
|------|------|-----|
| 1 | Optional `$GROK_HOME` durable exhausted-identity memo | Dual-auth soft; process-local shipped |

**Not ranked here:** operator git land; onto join/PR; import ledger; dual OAuth
SuperGrok; formal xAI import — separate tracks (`#6`–`#10`). Parked cosmetic /
optional invent items (`#2` B/C/D, `#11`, first-class tool names in `#12`) stay
out of this table until dogfood or operator re-rank.

**Tracking rule:** when a product slice ships, move lasting truth to FORK; keep
only **open next slice** here; demote soft gaps out of this table. Session board
(`feat:` / `impl:` / `plan:` leaves size 1\|2) mirrors this table — never wipe
foreign prefixes.

## Validate honesty

Focused filters for **remaining** open areas (and quick regression on shipped
neighbors). Full historical block:
[`doc/dev/campaigns/interject-todos-closed-2026-07.md`](doc/dev/campaigns/interject-todos-closed-2026-07.md).

**Open residual**

1. **UDAX T0–T6 regression:**
   `cargo test -p xai-grok-tools --lib -- toon json_to_toon dynamic_to_prompt free_text densify_mcp densify_structured task_output_handoff subagent_completed_handoff`
2. **Dual-auth (session ↔ console key hop + live re-bind + multi-add):**
   `cargo test -p xai-grok-shell --lib -- resolve_credentials enforce_disable_api_key store_and_load_round_trip fingerprint_is_not_raw_key multi_add`
   `cargo test -p xai-grok-sampler --lib -- rotate_ exhausted memo fingerprint hop_reason live_rebind`
   `cargo test -p xai-grok-pager --lib -- login_ dual_auth_hop_reason`
   `cargo test -p xai-grok-sampling-types --lib -- credit_exhausted`
3. **DOGE / hide_header (shipped polish regression):**
   `cargo test -p xai-grok-shared --lib -- hide_header`
   `cargo test -p xai-grok-pager-render --lib -- theme doge`
   `cargo test -p xai-grok-pager --lib -- hide_header context_bar`
4. **Plan soft-park A (shipped; B/C/D parked):**
   `cargo test -p xai-grok-pager --lib -- plan softer_park toast focus_plan plan_approval`
5. **session_reader / plan_validate / bulk_edit intercepts (A4 shipped; named tools parked):**
   `cargo test -p xai-grok-tools --lib -- session_reader plan_validate bulk_edit_policy implement_memory opencode edit`

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
