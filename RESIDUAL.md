# Open residual (human intent and unfinished honesty)

**D0 — open only.** Finished work lives in [`FORK.md`](FORK.md), process docs,
or code — not only here. Closed campaign history:
[`doc/dev/campaigns/interject-todos-closed-2026-07.md`](doc/dev/campaigns/interject-todos-closed-2026-07.md).

## Open

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

4. **OAuth SuperGrok ↔ console API key failover (D1+D2+D3+S1 + rate-limit hop shipped)**  
   **Shipped:** first-party resolve merge (session primary + console key
   failover; `preferred_method=api_key` reverses, including aux/web-search);
   identity rotate on **credit** and **plain HTTP 429** with **dual-host hop**
   (cli-chat-proxy ↔ `api.x.ai`, proxy header strip/restore, bearer
   stash/reinstall); **D3** process-local **credit** exhausted-fingerprint memo
   (1h TTL; preemptive skip before next attempt) + hop status chrome / toast
   (“… (credit exhausted)” vs “… (rate limited)”, fingerprints only — no raw
   keys); rate-limit hop observes temporary shared `grok-rate-limit` cooldown
   for the left identity (not the credit memo, so primary can return when cool);
   kill-switch clears console failover + dual-host metadata; xAI console keys
   in keyring `grok-build` + `provider_credentials.json` (env wins). User-guide
   `02-authentication` + `11-custom-models`.  
   **Also shipped (polish):** AuthManager **live re-bind without prior stash**
   (`session_bearer_resolver` durable; hop-to-session prefers stash then live
   re-bind; next turn re-resolves via `reconstruct_full_config`); **multi-add**
   console keys (`add_console_api_key` comma-list store; `grok login --api-key`
   multi-add; `grok login --list-api-keys` fingerprints only — never raw keys).  
   **Still open:** dual OAuth SuperGrok (S3) out of scope; optional `$GROK_HOME`
   durable memo (process-local only for now).  
   Plans: [`.agents/plans/plan-secure-key-failover.md`](.agents/plans/plan-secure-key-failover.md),
   [`.agents/plans/plan-rate-limit-failover.md`](.agents/plans/plan-rate-limit-failover.md).  
   Brief: [`doc/dev/research/secure-key-failover-2026-07-26.md`](doc/dev/research/secure-key-failover-2026-07-26.md).

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
2. **Dual-auth (D1–D3+S1 + live re-bind + multi-add):**  
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
