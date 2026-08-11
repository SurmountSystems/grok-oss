# Grok OSS fork notes

**Grok OSS** (`grok-oss`) is an **unofficial** open-source fork of
[xai-org/grok-build](https://github.com/xai-org/grok-build) (SpaceXAI’s Grok
Build CLI/TUI), maintained by [Surmount](https://github.com/SurmountSystems).

It is **not** affiliated with or endorsed by xAI / SpaceXAI. Trademarks and
product names belonging to xAI remain theirs.

**Why the fork exists:** upstream publishes under Apache-2.0 but **does not
accept external pull requests**. This repo accepts community PRs. If upstream
ever opens to outside contributions, Surmount intends to **open a PR** and try
to land the useful fork work there.

## Vision

| Pillar | Practice |
|--------|----------|
| **Faithful** | Absorb xAI monorepo exports after review; keep `xai-grok-*` paths for alignment |
| **Complete history** | Surmount `main` is the continuous product archive; xAI is a content feed |
| **Open** | Pull requests welcome **here** |
| **Distinct** | Product **Grok OSS**, binary **`grok-oss`**, clear unofficial labeling |
| **Compatible** | Config and sessions under **`~/.grok`** (shared with upstream if both installed) |
| **Superset** | Fork features sit **on top of** upstream behavior — never hollow out core agent logic |

## Git flow

Normal feature branches → pull request → **`main`**. Temporary tool branches
(`import/*`, `onto-xai/*`) are not a second main; they land via PR.

On **open PRs**, catch up with `main` by **merge**, not rebase (no force-push
while CI runs). Detail: [`docs/git-workflow.md`](docs/git-workflow.md).

## Remotes

```bash
git remote add xai-org https://github.com/xai-org/grok-build.git   # once
# origin → SurmountSystems/grok-oss
# xai-org → xai-org/grok-build
```

## Syncing with xAI

xAI publishes force-pushed snapshots (bot author, often orphan roots, sometimes
short “Synced from monorepo” chains). GitHub may say histories are “entirely
different.” **Expected.** Treat them as a **tree feed**, not shared ancestry.

**Maintainer jobs** (do not confuse them):

| Job | Script | Result |
|-----|--------|--------|
| **Import** — their tree into Surmount history | `./scripts/import-upstream-export.sh` | `import/*` review branch → PR to `main` |
| **Stack on tip** — our product commits on their tip | `./scripts/put-history-on-xai.sh` | `onto-xai/*` (real **cherry-pick**; no `MODE=overlay`) |
| **Join `main` into onto** — landable graph | `./scripts/join-main-into-onto.sh` | same tip; `main` becomes ancestor; **tree kept** (`-s ours`) → PR |

When histories keep breaking: **stack product on their tip**, then **join
Surmount `main`** (`-s ours`) so GitHub compare/PR works, then PR to `main`.
Detect: `./scripts/detect-upstream-export.sh` or `just upstream-detect`.

Full process: [`docs/upstream-history.md`](docs/upstream-history.md)
Import log: [`docs/upstream-import-log.md`](docs/upstream-import-log.md)
Onto log: [`docs/upstream-onto-log.md`](docs/upstream-onto-log.md)

**Never:** reset Surmount `main` to xAI; GitHub “Sync fork” that drops Surmount
commits; unsigned commits; bulk tree rewrites without review.

## What Grok OSS adds (divergence inventory)

Hierarchical: one line here → code or a linked doc for detail. Update this
list when you ship fork work.

### Product

- [x] **UDAX JSON→TOON (T0–T6)** — model-facing structured JSON densifies via shared `util/toon` policy (`GROK_TOOL_RESULT_FORMAT=auto|toon|json`). **T2** Dynamic tool results; **T3** MCP densify-before-truncate; **T4** `json_to_toon` tool; **T5** subagent handoff / task output / Text pure-JSON / SearchTool / SchedulerList / child task prompt via `densify_structured_text` (free text + ACP/MCP envelopes + on-disk `prompt_context.json` unchanged; fail-open); **T6** fail-open debug savings log (`N_json → N_toon`). Crate: `toon-format` 0.5 (`default-features=false`). Detail: [`doc/dev/research/udax-json-toon-2026-07-26.md`](doc/dev/research/udax-json-toon-2026-07-26.md)
- [x] **ULID helper** — `xai_grok_tools::util::ulid` mints 26-char Crockford base32 ids for new work/log/tool artifacts; task UUID v7 unchanged. Detail: [`doc/dev/research/ulid-helper-2026-07-25.md`](doc/dev/research/ulid-helper-2026-07-25.md)
- [x] **usage.jsonl (main + subagent turns)** — append-only per-session spend log at end of model turns (`session/usage_log.rs` ← `record_response_token_usage`); main rows `turn_type`/`agent_kind`=`main`; subagent/task rows `agent_turn` + subagent type + optional `work_ulid`; fail-open. Detail: [`doc/dev/research/usage-jsonl-2026-07-25.md`](doc/dev/research/usage-jsonl-2026-07-25.md)
- [x] **Binary / branding** — `grok-oss` (crate package still `xai-grok-pager-bin`); welcome, terminal/tab titles, resume hints, and docs say Grok OSS / `grok-oss`
- [x] **OpenRouter** — separate model option (`openrouter-grok-4.5`); login/logout; secret store; optional Zed credential probe (read-only)
- [x] **Multi-key OpenRouter** — comma lists / failover keys for credit + rate-limit rotation
- [x] **SuperGrok OAuth ↔ console API key dual-auth** — first-party resolve merge (session primary + console failover by default; `preferred_method=api_key` reverses); identity switch on **credit / SuperGrok Heavy usage-limit** and **plain 429** (session→key clears bearer; key→session via JWT in failover list); also switches API host (SuperGrok proxy ↔ `api.x.ai`); credit/allowance exhausted-fingerprint memo (process cache + durable `$GROK_HOME/exhausted_credits/`, 1h TTL; **console-key success clears**, **session success does not** — extras-paid SuperGrok 200s must not put SuperGrok back) + status/toast (“out of allowance” vs “rate limited”; labels only); when free SuperGrok period used percent `≥ 100%` + dual-auth, mark SuperGrok used up and prefer console key before the next request (no 402; clear on period reset); rate-limit switch uses temporary shared `grok-rate-limit` cooldown (not credit memo); kill-switch clears key failover + host metadata; console keys in keyring/`provider_credentials.json` + env/auth.json; **live re-bind without prior stash** (`session_bearer_resolver`); **multi-add** `grok login --api-key` + `--list-api-keys` (fingerprints only). **Also:** `[auth] auto_use_included_limits` rank+hop (prefer free SuperGrok period allowance before SuperGrok top-up dollars; sooner `reset_at` + headroom; ExhaustedAll→console; oauth/api_key pins fail-closed); **new/empty Grok home defaults this flag to true** (2026-08-03; explicit `false` preserved). Sticky-console meter honesty (no SuperGrok top-up sell while console is the live principal). **Multi SuperGrok OAuth:** two principals; second login does not wipe the first; doctor / list show both (role labels + fingerprints only); dual `/limits` rows; sibling billing poll for the non-active SuperGrok on the free-period-safe path. **SuperGrok Heavy multi-slot load:** when base JWT is live/fresher and multi-slot is stale/exhausted, ranking + doctor prefer the **live base** (not blind multi-slot); enrichment upsert keeps multi-slot in lockstep with base. Plans: [`.agents/plans/plan-secure-key-failover.md`](.agents/plans/plan-secure-key-failover.md), [`.agents/plans/plan-rate-limit-failover.md`](.agents/plans/plan-rate-limit-failover.md), [`.agents/plans/plan-auth-preferred-roles-failover.md`](.agents/plans/plan-auth-preferred-roles-failover.md).
- [x] **Free SuperGrok period always before credits (chrome + observe, 2026-08-07; protect 2026-08-08; intent vs settlement labels 2026-08-09)** — token economy: free SuperGrok period → SuperGrok $ credits → console. **Smoking gun:** sticky status pin must not paint `console · $N` while live free SuperGrok period has headroom (exhaust memo blocked by live free-period poll; Design A compact stays intent free SuperGrok period `%`). **Compact free SuperGrok period label (Work C):** free SuperGrok period paints `free SuperGrok period · N%` (cold `free SuperGrok period · ...%`); SuperGrok dollar credits still `SuperGrok extras · $N`; console still `console · $N`. **Footer secondary team meters (2026-08-09 operator fix; quiet while free SuperGrok period has room, same day):** while free SuperGrok period is primary with room, prompt footer omits long team prepaid / Grok Build class copy (team wallets on `/limits`; compact stays free SuperGrok period). After free SuperGrok period is full, SuperGrok-live secondary may use `not the active spend path: team prepaid remaining $N · Grok Build class $M` (cold: `not the active spend path: loading team prepaid...`); never mean "paying team prepaid now." Prior `Team settlement:` was misread as active pay; always-on not-active-spend while free SuperGrok period had room was messy. Consumer surface off (`usage_visible` false for team AuthMeta) still hides footer secondary team $ while compact free SuperGrok period paints. **`limits --json` / human `/limits`:** additive `activeDriver` / **Active:** line (`supergrok_free_period` | `supergrok_extras` | `console_key`) stays intent only. Settlement honesty: team Grok Build / OAuth class can rise under SuperGrok without free-period move (C6 + flat+settlement note; never called SuperGrok dollar credits). Rank still omits console primary under auto_use while free period has headroom. **C4 (server free-period debit) is not client-fixed:** operator must file the paste-ready ticket [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md); multipoll addendum [`.agents/reports/c4-ticket-addendum-2026-08-08-multipoll.md`](.agents/reports/c4-ticket-addendum-2026-08-08-multipoll.md). Dense multipoll flat-poll + `flatPollUnprovenDebit` on `limits --json` **shipped**. **Operator honesty + optional hard block (2026-08-08; dogfood default flipped same day):** default **allows** sampler turns under free SuperGrok period headroom + flat-poll unproven debit (loud honesty on `/limits` / multipoll / doctor / turn warn). Opt-in hard block: `[auth] allow_spend_when_free_period_debit_unproven = false` (or env `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN=0`). Report: [`.agents/reports/impl-limits-before-credits-2026-08-07.md`](.agents/reports/impl-limits-before-credits-2026-08-07.md); protect: [`.agents/reports/impl-limits-over-credits-protect-2026-08-08.md`](.agents/reports/impl-limits-over-credits-protect-2026-08-08.md); unblock default: [`.agents/reports/impl-unblock-flat-poll-default-2026-08-08.md`](.agents/reports/impl-unblock-flat-poll-default-2026-08-08.md); C4 hard address: [`.agents/reports/impl-c4-address-hard-2026-08-07.md`](.agents/reports/impl-c4-address-hard-2026-08-07.md); Work C chrome: [`.agents/reports/impl-work-c-meters-chrome-2026-08-09.md`](.agents/reports/impl-work-c-meters-chrome-2026-08-09.md); team secondary footer honesty: [`.agents/reports/impl-team-settlement-chrome-vs-limits-2026-08-09.md`](.agents/reports/impl-team-settlement-chrome-vs-limits-2026-08-09.md).
- [x] **Billing meters (two halves; core shipped)** — meters stay distinct: personal SuperGrok **included weekly** ≠ SuperGrok **dollar extras** ≠ **console team prepaid** (Half B = team API prepaid / postpaid / USD series; **not** the browser Platforms → Grok Business **licenses** messages/conversations page) ≠ second SuperGrok OAuth principal. **Honesty:** multi-pool / “paying double” is xAI product billing structure (docs + surfaces), **not** a missing code merge of pools on this branch. License page zeros are a **non-goal** for CLI dogfood (no license API client; no scrape); **zeros expected** (CLI SuperGrok does not drive seat counters). Product dogfood proof = `limits` / `/limits` team postpaid OAuth **Grok Build class** + usage series + browser team Usage (`.../usage`), not licenses. Doctor + user-guide troubleshooting + sharper `/limits` note (2026-08-07). **Half A shipped:** dual SuperGrok `/limits`, sibling poll, footer honesty for included weekly + SuperGrok $ extras. **Half B core prepaid shipped:** management key (keyring URL `https://management-api.x.ai`) + `[endpoints] management_team_id` + hermetic `GET …/billing/teams/{team_id}/prepaid/balance`; footer `Console key · team prepaid: $N` when console live; **SuperGrok live also surfaces team prepaid $** (or loading/unavailable gap when Management path active); SuperGrok-live footer also surfaces **`team Grok Build class: $N`** when postpaid OAuth class is known (distinct from prepaid); `/limits` Console puts OAuth / Grok Build class near top when known; Balance always under Console API even when `console.isLive=false`; honest **distinct** gaps (`no management key` | `no management team id` | `loading team prepaid...` | `team prepaid unavailable`). `/usage` surfaces team prepaid on SuperGrok live when known. **Usage series cadence shipped (P2, 2026-08-07):** Management `POST …/usage` analytics (OAuth / Grok Build class + API class + top descriptions) refreshes on background `FetchBilling`, TUI `/limits` open, and CLI `grok limits`, with the same ~60s process soft cache as prepaid/postpaid (explicit open/collect force-busts; no unbounded spam). `/limits` Console attaches warm series when known. **Still open (not shipped):** full browser-style series **charts** UI (text series totals shipped). Live prepaid dogfood (2026-08-02): `total.val` → **$340** is correct; console dashboard ~$1317 is defaultCredits/composite, not prepaid wallet (keep meters distinct). See `RESIDUAL.md` §4. Reports: `.agents/reports/impl-supergrok-live-team-usage-2026-08-04.md`, `.agents/reports/impl-grok-business-license-zeros-vs-team-usage-2026-08-07.md`, `.agents/reports/impl-p2-usage-series-fetch-billing-2026-08-07.md`.
- [x] **Keyring login time-box + fail-loud + secure fallback + TTY progress** — OS keyring get/set/delete wall-clock budget (`KEYRING_OP_TIMEOUT`); interactive `grok login --api-key` / OpenRouter login require a **secure** backend (primary platform store, then on Linux automatic **keyutils** fallback when Secret Service times out/errors). TTY stderr progress counts seconds up to **2× timeout (~6s)** during store RMW+write (suppressed non-TTY / env short-circuit). Only if **all** secure backends fail → clear error, **no** silent `provider_credentials.json` secret dump. File mirror only after successful secure write. `GROK_CREDENTIALS_FORCE_FILE` = tests/CI only (not user recovery).
- [x] **Economic mode** — soft-cap effective context at the Grok 4.5 long-context price cliff (~200k); `/economic-mode`; settings default on. Separate from Token Economy implement-effort caps (see next).
- [x] **Token Economy (full product; further TE options plan parked 2026-08-04)** — four pillars: (1) implement-loop effort 1–5 policy on all entry paths (auto-run + human `/implement`): **lock** (`lock_implement_effort`, optional) and **min floor** (`min_implement_effort`, default **1**) always apply when set; when `[ui] economic_mode` and `cap_implement_effort_when_economic` (default true), hard ceiling (default max **3**) and desired inject **2** when missing; order lock → desired/present → min floor → economic ceiling; toasts on rewrite; default min 1 keeps prior behavior (set min **2** for always-a-reviewer); (2) free SuperGrok **billing period** linear-burn pacing on credit/status + `/limits` + `/usage` (“ahead/behind **linear burn**”; omit without bounds; console-live honesty; never dollar-ize period %); (3) double-entry local `usage.jsonl` book vs Management remote samples on `/spend` and a `/limits` section with gap honesty when local cost is missing; (4) durable store **`$GROK_HOME/grok_oss.db`** (not session sqlite; fail-open; multiproc busy timeout; no secrets; additive schema). Config table `[token_economy]`. Modules: shell `token_economy/` + `grok_oss/`.
- [x] **Auto-compact default 95% + live-apply** — stock Grok 4.5 catalog omits a per-model undercut (was 80); remote `models_cache` undercuts on stock models are dropped so the product default applies; user session/env still win; banner shows usage **and** configured threshold. Settings commit live-applies to open sessions (`restart_required: false`): disk persist → ACP `x.ai/auto_compact_threshold_changed` → `SessionCommand::SetAutoCompactThreshold` → CompactionConfig Cells (same write path as model switch). Live-apply pushes the **committed Settings value** (race-safe vs disk); env `GROK_AUTO_COMPACT_THRESHOLD_*` wins again on the next full resolve (spawn / model switch). Detail: `docs/dev/research/rca-auto-compact-early-fire.md`
- [x] **Auto-run `/implement`** — after a successful turn, queue a follow-up implement block when present; **appends** after any already-queued prompts (does not drop them). Product may rewrite implement-loop `--effort` via Token Economy (lock / min floor always when set; economic ceiling + desired inject when caps are on; toast on rewrite)
- [x] **Shared rate limits** — crate `grok-rate-limit` (Surmount name, not `xai-`); cooldowns under `~/.grok/rate_limits/`; optional `GROK_DISABLE_SHARED_RATE_LIMIT=1`
- [x] **Updates** — no xAI auto-update channel by default (wrong product). `grok-oss update --check` compares to Surmount `main`. Escape hatch: `GROK_OSS_ENABLE_XAI_UPDATER=1`
- [x] **`/rebuild` + `grok-oss rebuild`** — from a local checkout: `just install` (or fixed cargo argv), verify package version + git SHA, soft-signal leaders via `RelaunchForUpdate` with **SHA-aware** identity (same semver + newer SHA accepts), write cooperative relaunch request + `SIGUSR1` **all other live product TUIs** in `active_sessions` so they re-exec onto the new binary (not only the invoker), inventory live sessions, self re-exec this TUI with the same session. Mid-turn uses cancel + `canceled_turn_resume`. Not SpaceXAI download.
- [x] **Soft interject only + Enter cue honesty (2026-08-09 Work A)** — mid-turn interject (Ctrl+Enter / terminal alts, queue `[Interject]`) injects into the **current** turn and **never cancels**. Cancel is Esc/stop only. Shell contracts: `interject_contract_*` tests. Do **not** re-unify user mid-turn steer on `SendPromptNow` (cancel-and-send). **Composer footer Enter label** follows pure `enter_prompt_mode`: clean idle → `send`; primary turn running → `queue`; empty + mid-turn with queued follow-up → `interject`. **Background subagents alone do not force Enter:queue** (operator 2026-08-09): primary idle + live children → Enter **sends** a normal main turn; status still-running cue + pause/stop stay; no `· Enter queues` / force-drain for children-only. **Parked sendable-wait exception (intentional):** while the agent is **blocked waiting** (task/subagent) **and the queue is empty**, plain Enter with text may still cancel-and-send to unblock immediately — not soft Interject. User-guide `03-keyboard-shortcuts`, `16-subagents`. Reports: [`.agents/reports/impl-work-a-composer-enter-cue-2026-08-09.md`](.agents/reports/impl-work-a-composer-enter-cue-2026-08-09.md), [`.agents/reports/impl-enter-queue-when-only-subagents-2026-08-09.md`](.agents/reports/impl-enter-queue-when-only-subagents-2026-08-09.md).
- [x] **Todo board survives auto-compact** — pager no longer clears the UI todo list on `AutoCompactCompleted` (Resources still held the board; UI wipe was a lie). Contract: `auto_compact_completed_preserves_todo_board`.
- [x] **plan.json honesty + resume board** — compact writes the **live** Resources `TodoState` to `plan.json` (no empty wipe). Resume loads `plan_state` again and re-emits ACP `Plan` from Resources / `plan.json` fallback (`RestoreTodoBoard`). Real SoT: in-memory Resources + on-disk **`resources_state.json`** (bridge path is named `tool_state.json` but registry rewrites to sibling `resources_state.json`); `plan.json` is a mirror + resume fallback. User-guide `17-sessions` documents both.
- [x] **Auto-seed user asks as todos** — real user turns seed protected `ask:<prompt_id>` (cap 20, truncated content); `ask:` is keep-unless-mentioned on `merge: false`. Helpers + tests in `xai-grok-tools` todo module.
- [x] **Default agent uses the todo board** — base `prompt.md` teaches `todo_write` (Planning section, gated on plan tool): multi-step / `feat:` / `bug:` / merge upsert / protected prefixes / red/green TDD for user-reported bugs & features / mark complete / Ctrl+T board. First empty→non-empty Plan auto-opens the todo pane once. Fork/copy includes `resources_state.json` (not only `tool_state.json`).
- [x] **Plan approval CTAs + decisive Revise (2026-08-09; decision surface P1 / revise loop P2 2026-08-10)** — primary path is **clickable** footer / side-panel buttons (mouse primary); keys `a`/`A`/`?`/`s`/`q` when the plan panel has empty prompt focus. Outcomes: approved / approved+notes / `"questions"` / cancelled / abandoned. **Revise** (`s` / mouse footer) is **decisive**: immediate ACP `cancelled` + clear park/panel + toast “Revision sent…”, not a silent intent flip. **Empty freeform Enter never approves** (use mouse Approve or empty-prompt `a`). Soft-park present status **“Plan ready. Side panel open”**; after Revise/Clarify, **“Revising plan...”** / **“Waiting for updated plan...”** with no idle CTA re-arm until a new `exit_plan_mode`. Clarify / Notes stay focus-only for typed input. Soft-park footer CTAs are hit-tested (draft durable); card + empty placeholder are not fake menus; FileBacked preview/card re-read live `plan.md`. **Never** freeform chat “reply approve/revise.” Main thread (agent L1) stays **modal-free** for typing. User-guide `19-plan-mode`. Reports: [`.agents/reports/impl-plan-revise-stuck-2026-08-09.md`](.agents/reports/impl-plan-revise-stuck-2026-08-09.md), [`.agents/reports/impl-plan-panel-revise-test-2026-08-09.md`](.agents/reports/impl-plan-panel-revise-test-2026-08-09.md), [`.agents/reports/impl-p1-plan-decision-surface-2026-08-10.md`](.agents/reports/impl-p1-plan-decision-surface-2026-08-10.md), [`.agents/reports/impl-p2-revise-loop-chrome-2026-08-10.md`](.agents/reports/impl-p2-revise-loop-chrome-2026-08-10.md). Residual soft: agent-written `plan.md` may still invent freeform menus.
- [x] **Same-batch plan write + `exit_plan_mode` (stale body fix, 2026-08-09)** — mixed multi-tool batches run non-exit tools **to completion first**, then prepare/run `exit_plan_mode` (`split_tool_batch_before_exit_plan_mode`), so park and tool re-read see post-write `plan.md`. Pager FileBacked re-read alone could not fix a co-batched write that had not executed yet. Report: [`.agents/reports/impl-plan-stale-after-exit-plan-mode-2026-08-09.md`](.agents/reports/impl-plan-stale-after-exit-plan-mode-2026-08-09.md).

### Packaging and build

- [x] **AUR** sources under `packaging/aur/`
- [x] **Nix flake** — `nix build .#grok-oss`, dev shells (human packaging, not GHA release artifacts)
- [x] **justfile** — `just check` / `just ci` full quality gate; `just test` for the cargo quality suite
- [x] **release-dist debug sidecar** — `just build-dist` / `just install-dist` build with `--profile release-dist` (strip=false, debug=1), extract DWARF to `grok-oss.debug` via `scripts/extract-debug-sidecar.sh`, strip the binary, embed GNU debuglink. Plain `just install` stays local `--release` + strip (no sidecar).

### Process

- [x] **Process docs hierarchy** — D0 residual open-only; D1 AGENTS; D2 logs under `docs/upstream-*` and `doc/dev/campaigns`; D3 research / skill `references/`
- [x] **Upstream tooling** — detect / import / put-history / **join-main-into-onto** / sync scripts; scheduled export watch workflow
- [x] **Onto land path** — after product is on their tip, join Surmount `main` with `merge -s ours` so the tip is PR-able (`docs/upstream-history.md`, `just upstream-join-main`)
- [x] **PRs accepted** — CONTRIBUTING / this fork
- [x] **Parent = HITL only** — main thread (agent **L1**) goals/spawn/reports/human git; research + implementation in subagents (**L2**); L2 may spawn specialists (**L3 max**, no deeper). Hard stop on CI / multi-file. **Also** / “this too” = additive second slice (do not kill healthy in-flight work). Plan approval = product CTAs only (not freeform chat approve). Red/green TDD for behavior bugs. See [`AGENTS.md`](AGENTS.md)
- [x] **Subagent worktree policy** — prefer isolation none; product default
  `[subagents] allow_worktree = false` (empty config force-none; opt in with
  `true`). Spawn still forces none when false. User-guide migration notes in
  `05-configuration` + `16-subagents`. Host skills dual-pin todo namespaces
  (`plan:*` / `impl:*` / …) + worktree optional. Campaign:
  `doc/dev/campaigns/operator-orchestration-2026-07.md`
- [x] **`/execute-plan` honors `allow_worktree`** — host skill defaults to
  shared-cwd protocol (serial/disjoint writers, on-disk reviews, no worktree
  path handoffs); worktree only when policy allows; fall back if spawn forces
  none or create fails. Report:
  `doc/dev/research/execute-plan-no-worktree-2026-07-24.md`
- [x] **Todo levels product surface** — `todo_write` accepts optional
  `priority` + `meta` (`kind`, `parentId`, `namespace`); `merge: false`
  keep-unless-mentioned for protected prefixes (`plan:`, `impl:`, `pr-`,
  `recon:`, `residual:`, `ask:`, `feat:`, `bug:`). Feature suggestions use
  `feat:`; user-reported bugs use `bug:` (session board; not durable residual
  unless campaign-ranked). Red/green TDD for user-reported bugs/features.
  Light `[kind]` badge in todo pane. Report:
  `doc/dev/research/todo-levels-product-2026-07-24.md`
- [x] **Todo fib leaves + weighted progress** — optional `size` on items
  (only **1|2**; `meta.size` fallback); reject size on parents with children;
  progress = Σ leaf sizes (legacy item counts when no sizes); badge
  `N/M pts` in points mode; tool output `progress` + merge:false archive
  warning; prompt + tool blurb teach merge-only + fib structure. Report:
  `doc/dev/research/todo-progress-fib-2026-07-26.md`
- [x] **Cleared todo archive** — items dropped by `merge: false` (unprotected
  unmentioned) or ask-cap prune land on a capped `cleared_todos` ring on
  `TodoState` (max 200; Resources serde). Active board / Plan / todo pane stay
  active-only. Detail:
  `doc/dev/research/cleared-todos-archive-2026-07-25.md`
- [x] **Session notes channel** — `/note` stores operator mid-session
  annotations that are **not** pending main-turn prompts (session-local
  store; list via bare `/note` / `/notes`; count on `/tasks`). Does not
  replace short on-disk L2 reports. Report:
  `doc/dev/research/notes-channel-2026-07-24.md`
- [x] **Git recon depth** — host skill `/git-recon` (status → route →
  conflict ≤3 buckets → stage → human-sign → land; never agent-commit);
  product `scripts/recon-status.sh` + `just recon-status` (read-only probe);
  pin in `FORK_PATHS` + `assert-process-pins`; optional
  `.grok/workflows/git-recon-status.rhai`. Reports:
  `doc/dev/research/git-recon-skill-created-2026-07-24.md`,
  `doc/dev/research/recon-status-script-2026-07-24.md`
- [x] **Prefer Rust tools over ad-hoc Python** — standing preference + inventory
  pin; **A1** steers; **A2** implement-memory embed+intercept; **A3** in-process
  bulk-edit policy on `search_replace` **and OpenCode `edit`**
  (`util/bulk_edit_policy`: storm N=5/T=120 + optional `GROK_DENY_REPLACE_ALL=1`;
  host hook still complementary); **A4** `util/plan_validate` (full
  validate-plan.py + bash intercept) and `util/session_reader` (CLI intercept +
  Claude inert list/show + **Codex** SQLite state + rollout jsonl + **Cursor**
  CLI `store.db` / desktop `state.vscdb` discovery+read; fail closed; fixture
  tests); **skill-text demotion** (host): resume-session / implement /
  execute-plan document Grok intercept while keeping allowlisted CLI form;
  review + zed-settings drop non-intercepted `python3` heredocs (`write` /
  `jq` / native edit). Legitimate remaining `python3` in skills = allowlisted
  intercept surface + office/PDF + anti-patterns (“don’t invent uuid”).
  **Parked (no dogfood):** first-class named memory/plan-validate GrokBuild
  tools — intercept is the product surface; do not invent unless discoverability
  breaks. Research:
  [`doc/dev/research/python-to-rust-tools-2026-07-26.md`](doc/dev/research/python-to-rust-tools-2026-07-26.md)
- [x] **Hide header + DOGE theme** — `[ui] hide_header` (default **false**) zeros
  top agent status bar, welcome location top bar, and dashboard location header
  only (in-app chrome). Theme id `doge` (display “DOGE”, no aliases): pure
  black/white + 8 pure primaries. **Default theme** when unset and on auto-dark
  (auto-light stays GrokDay; switch back with `theme = "groknight"` or
  `/theme`). Docs: user-guide `06-theming`.
- [x] **Window titles on by default** — product always manages OSC window titles
  when `[ui.notifications.title] enabled` (default **true**: session name +
  activity + agents + `grok-oss` brand; Welcome uses a real session name, not
  a blank). Titles flush on the live TTY path (not draw-deferred only). Distinct
  from `hide_header` (in-app bars only). **No** `[ui] hide_title_bar` (removed;
  stale config key ignored). Opt-out = `[ui.notifications.title] enabled =
  false` only. Never emit empty window-title OSC. User-guide `05-configuration`
  / `06-theming`.
- [x] **DOGE pure 8-colour palette** — durable pure palette (`#000000`…
  `#FFFFFF` + eight primaries) as product truth for `doge`; hard-threshold
  quantise + optional Floyd–Steinberg helper in
  `xai-grok-pager-render::theme::doge`; user-guide DOGE section. Project note
  (not an ECMA standard):
  [`doc/dev/specs/doge-pure-8-colour-2026-07-26.md`](doc/dev/specs/doge-pure-8-colour-2026-07-26.md).
- [x] **DOGE polish (Wave 2)** — context-bar **solid DOGE steps** (no mid-gray
  lerp); pure-primary **`doge.tmTheme`** for DOGE syntax; `hide_header`
  extended to welcome + dashboard headers.
- [x] **DOGE rails + roles + activity glyphs** — Human prompts: static green
  left `┃` (`accent_user`). Agent messages: magenta left rail
  (`accent_running`) **only while the turn is active**; finished agent
  scrollback has no coloured rail. Yellow context/time rails use **striped**
  dashed glyphs (not solid pink/green). Composer caret: slow green filled-box
  ↔ hollow-box blink (paint keeps graphemes; hardware cursor hidden while
  box caret paints; **Human green only**, never agent magenta). Left activity
  throbber under DOGE is a **dashed marquee** / agent-magenta activity cue;
  right-side status sparkle stays classic density frames (must not share the
  left marquee). Gray/alpha scrub: DOGE `blend_color` solid-steps (no mid-channel
  RGB invent); finished labels keep pure role primaries. Role map: Green=Human,
  Magenta=Agent (active), Yellow=context/time, Cyan=system/limits/credits.
  External SoT:
  [0001_DOGE.md](https://github.com/SurmountSystems/specs/blob/main/0001_DOGE.md).
  User-guide `06-theming`; annex
  [`doc/dev/specs/doge-pure-8-colour-2026-07-26.md`](doc/dev/specs/doge-pure-8-colour-2026-07-26.md).
  Soft open: optional rename of token ids still named `gray*` (values already pure).
  **Do not invent** unnamed glyph colours from screenshots (e.g. “little guy”);
  caret residue ≠ lower-left throbber ≠ Clear finished.
- [x] **Stuck Retrying / network-switch graceful retry** — sticky yellow Retrying
  chrome soft-reconnects on stream start (`RetryState::StreamResumed` → reason
  `reconnecting`, keeps attempt N; not zombie "Waiting for response…" for the
  headers/TTFB window). Real stream content still clears via `handle_update`.
  Stream response-headers / first-byte timeout default **120s**
  (`GROK_STREAM_HEADERS_TIMEOUT_SECS`; not connect 10s, not post-headers idle).
  Cancel-aware shared cooldown wait; short transport footer labels
  (`timed out` / `connection interrupted` + `· next try in Ns` backoff; not
  opaque `Transport error: error`).
- [x] **Clear finished todos** — pane chrome compact **`[−]`** icon (U+2212;
  not empty-set, not a long "Clear finished" string). Paints in the todo
  header next to close when the todo board is **open** and finished rows
  exist (focused **or** unfocused — findable while looking at scrollback/
  tasks). No paint when the board is hidden or nothing is finished. Quiet
  idle / stronger hover; never neon green or agent magenta.
  + `/clear-completed-todos` archives completed/cancelled
  (`ClearedReason::UserClearCompleted`); not `h` hide-done and not
  `merge: false` wipe. Hints/slash still say Clear finished. Optional
  focused `X`. Layout still reserves a chrome gap above the todo body so
  clear **must not** paint into tasks model/timer / `[↗]` / `[x]` (compact
  smash case). Slash reserved in pager `SHELL_RESERVED` (`shell_collision`
  contract). User-guide `03` / `04` / `17`.
- [x] **Click tasks top-right chrome → open subagent** — single click on tasks
  pane **model + elapsed + `[↗]`** open chrome calls the same
  `open_subagent_fullscreen` path as Enter / Ctrl-F / double-click. Kill `[x]`
  stays separate. Tasks open/kill hits win z-order over Clear finished.
  User-guide `16-subagents`.
- [x] **“Worked for …” one live line** — parked turn marker is **one row per
  prompt turn**; mid-park epoch ticks and re-parks **refresh elapsed in place**
  (no append-per-tick scrollback flood). Still-running counts stay on the
  status cue, not stacked “Worked for” rows. User-guide `20-background-tasks`.
- [x] **Status-bar limits meter** — compact SuperGrok `N%` (or cold `...%`) /
  console prepaid/gap on the top status row for Build sessions
  (including team dual-auth when consumer slash surface is off); gateway chat
  still hides it. Click opens `/limits` (same data path as slash; not a second
  billing system).
- [x] **Always-on bubble copy + one-click copy** — selection-box / plan top-bar /
  prompt draft / per-bubble `⧉` (`bubble_copy_buttons` default on) reuse the
  clipboard stack; Policy A keeps selection ⧉ off bubble-owned blocks only.
  Hover on copy chrome requests OSC 22 **pointer** cursor (same path as links;
  hosts without OSC 22 keep the default arrow). Settings **Bubble copy buttons**
  (Appearance) toggles `[scrollback.display] bubble_copy_buttons` in pager.toml.
- [x] **Session recap in Settings** — search `recap` for **Auto session recap**
  (`[ui.notifications] session_recap`), **Auto recap after (seconds)**, and
  **Master session recap** (`[features] session_recap`, restart-required). Also
  **Cancel subagents with turn** sticky enum under Agent.
- [x] **btw Done-panel keys in user-guide** — focused `y` copy full thread, `a`
  follow-up same session, Esc dismiss (`04-slash-commands`).
- [x] **Plan approval soft park → auto-open side panel** — `exit_plan_mode`
  soft path parks durable approval, keeps the live draft, and **auto-opens**
  the non-capturing plan **side panel** (same surface as `/view-plan`) with
  full borders + approval footer CTAs. **Three surfaces:** (1) side panel
  (borders + Approve / Notes / Clarify / Revise / Quit), (2) soft-park strip
  CTAs when the panel is dismissed or too small to paint, (3) transcript plan
  card (preview pointer only, not a fake button menu). Never silent empty
  chrome. Live park status: **“Plan ready. Side panel open”** (not idle
  **“Plan written. Click or /view-plan”**). L1 typing stays free (printable
  chars → composer). `/view-plan`, status click, `ShowPlan` still reopen if
  the panel was dismissed. Force fullscreen: `[ui] plan_approval_park =
  "modal"`. Product CTAs only; no freeform chat approve. Design note
  (historical option A):
  [`doc/dev/research/plan-modal-softer-park-2026-07-26.md`](doc/dev/research/plan-modal-softer-park-2026-07-26.md)
  User-guide `19-plan-mode`.
- [x] **Plan decision surface (P1, 2026-08-10)** — after soft-park present,
  CTAs are armed without an extra open; status **“Plan ready. Side panel
  open”**. **Empty freeform Enter never approves** (no text, no line comments,
  no images → no-op on Prompt). Empty-prompt **`a`** with side panel open
  still Approves; soft-park without panel, bare `a` types into composer
  (mouse strip owns decide). Footer never shows `Enter:approve` on empty
  Prompt. User-guide `19-plan-mode`. Report:
  [`.agents/reports/impl-p1-plan-decision-surface-2026-08-10.md`](.agents/reports/impl-p1-plan-decision-surface-2026-08-10.md).
- [x] **`exit_plan_mode` present ≠ operator Approve (2026-08-10)** — bare tool
  result is present-only (“NOT operator approval”; never “has been approved” /
  “start coding”). Real Approve synthesizes panel-CTA language after a plan
  panel click. No interactive client uses honest no-panel copy (not false
  Approve). **Always-approve** permission mode skips tool permission prompts
  only; it does **not** auto-click plan CTAs. User-guide `19-plan-mode`,
  `22-permissions-and-safety`. Reports:
  [`.agents/reports/impl-plan-auto-approved-false-2026-08-10.md`](.agents/reports/impl-plan-auto-approved-false-2026-08-10.md),
  [`.agents/reports/impl-ci-two-test-fails-2026-08-10.md`](.agents/reports/impl-ci-two-test-fails-2026-08-10.md).
- [x] **Sticky plan decision after Approve / Quit (2026-08-10)** — session flag
  `plan_decision_resolved` blocks re-arm of “Plan ready” / Approve strip after
  one decisive Approve or Quit, even when shell `CurrentModeUpdate` clears
  pending while plan mode is still active. Re-arms only on a **new**
  `exit_plan_mode` present. Revise / Clarify do not set sticky. User-guide
  `19-plan-mode`. Report:
  [`.agents/reports/impl-plan-multi-approve-still-broken-2026-08-10.md`](.agents/reports/impl-plan-multi-approve-still-broken-2026-08-10.md).
- [x] **Revise/Clarify in-flight chrome (P2, 2026-08-10)** — after decisive
  Revise or Clarify unparks, `plan_feedback_in_flight` blocks idle “Plan written.
  Click or /view-plan” status and local idle decision re-park until a new
  `exit_plan_mode` present. Status paints **“Revising plan...”** /
  **“Waiting for updated plan...”**. Freeform while the channel is closed
  queues as a normal follow-up with an honest toast (never silent fail).
  User-guide `19-plan-mode`. Report:
  [`.agents/reports/impl-p2-revise-loop-chrome-2026-08-10.md`](.agents/reports/impl-p2-revise-loop-chrome-2026-08-10.md).
- [x] **Revise barren-wait landing (2026-08-10)** — after decisive Revise/
  Clarify: always push a human scrollback line (notes or **“Revise the plan”**
  / **“Clarify the plan”**); clear composer (no ghost draft **Enter:queue**);
  idle in-flight keeps Revising status; busy rewrite paints real turn status
  (thinking/tools/cancel) instead of exclusive barren wait; dead reverse-
  request channel still Interjects a rewrite kick. P1 empty Enter / sticky
  Approve and P2 no idle re-arm preserved. User-guide `19-plan-mode`. Report:
  [`.agents/reports/impl-revise-barren-wait-2026-08-10.md`](.agents/reports/impl-revise-barren-wait-2026-08-10.md).
- [x] **Composer caret Human green + no residue (P3, 2026-08-10)** — software
  box caret uses `accent_user` (Human green under DOGE), never agent magenta.
  After arrow / home / end move, previous cells repaint as normal text (no
  leftover green plate or solid `█`). Solid block glyph only at **buffer end**;
  mid-draft spaces keep reverse-plate styling (no solid green `█` stuck in the
  line). Mid-draft **letters**: solid half reverse plate (readable canvas ink);
  empty half **`text_primary`** (normal text colour) — not neon green letter
  ink. OSC 12 session cursor stays `accent_user`. User-guide `06-theming`,
  `03-keyboard-shortcuts`. Report:
  [`.agents/reports/impl-p3-green-letter-caret-2026-08-10.md`](.agents/reports/impl-p3-green-letter-caret-2026-08-10.md).
- [x] **Composer Ctrl+Home / End / Page buffer nav (2026-08-10)** —
  `Ctrl+Home` / `Ctrl+PageUp` → start of whole draft; `Ctrl+End` /
  `Ctrl+PageDown` → end of whole draft. Bare `Home` / `End` stay visual-row
  local. User-guide `03-keyboard-shortcuts`. Report:
  [`.agents/reports/impl-composer-green-char-and-nav-2026-08-10.md`](.agents/reports/impl-composer-green-char-and-nav-2026-08-10.md).
- [x] **Lower-left activity throbber agent magenta** — tool-running braille
  spinner and idle “subagents still running” concentric icon use
  `accent_running` (magenta under DOGE), not success green and not system cyan.
  Distinct from composer caret (Human green) and Clear finished (quiet
  secondary). User-guide `06-theming`.
- [x] **Plan approval panel SoT = live `plan.md`** — FileBacked preview re-reads
  session `plan.md` on open / body resolve (frozen reverse-request snapshot is
  fallback only). Product CTAs only; no freeform chat approve. User-guide
  `19-plan-mode`.
- [x] **Fearless global pause + status `[pause]`/`[resume]` (Work B, 2026-08-09)** —
  `Ctrl+Shift+Space` toggles pause of **all** in-process agent sessions (not
  only the focused one): cancels running turns, holds queue drain, toast tracks
  paused duration + sessions held. Resume re-queues interrupted mid-turn prompts
  **once** and drains true pending work only; finished agents are never
  re-spawned; empty resume is a no-op. Status row **`[pause]` / `[resume]`**
  (quiet gray, `text_primary` hover) dispatches the same global pause action
  (never hard cancel). Visible when primary turn is live, subagents > 0, or
  global pause is active (resume stays discoverable after cancel-all). Bare
  Space and voice `Ctrl+Space` unchanged. User-guide `03-keyboard-shortcuts`,
  `17-sessions`. Reports: [`.agents/reports/impl-work-b-pause-stop-chrome-2026-08-09.md`](.agents/reports/impl-work-b-pause-stop-chrome-2026-08-09.md),
  [`.agents/reports/impl-pause-stop-verify-or-fix-2026-08-09.md`](.agents/reports/impl-pause-stop-verify-or-fix-2026-08-09.md).
- [x] **Hard stop status `[stop]` + soft stop chord (Work B, 2026-08-09)** —
  Status row **`[stop]`** (gray at rest, `accent_error` hover) is **hard cancel
  only** (`CancelTurn`). Visible when primary turn is live **or** subagents > 0
  (not monitors-only). Idle + live subagents: stop opens existing subagents
  panel / kill path without inventing a parent cancel. **Soft stop** remains
  **keyboard only** (`Ctrl+Shift+S`): after the current top-level turn finishes
  (success or terminal fail), automatic queue drain stops; subagents for that
  turn may finish with it; does **not** cancel mid-flight. **No soft-stop
  button** (intentional). Toast + status chrome for soft-stop armed vs queue
  held. User-guide `03-keyboard-shortcuts` § Pause vs stop. Reports: same Work B
  pair as global pause above.
- [x] **Continue interrupted turn on restart** — cancel-resumable mid-turn
  persists a session marker (`canceled_turn_resume.json`); reopening the session
  re-queues once when `[ui] resume_canceled_turn_on_restart` is on (default
  true). Toast **“Continuing interrupted turn...”** (plain name; not `/resume`
  session pick). **Writes on:** Esc/stop, graceful process quit (SIGTERM /
  first signal → Quit / `/exit`, including default `killall`), and `/rebuild`
  mid-turn cancel. **Does not** write on clean success, fearless pause, soft
  stop, or **SIGKILL** (`kill -9` — no userspace handler). Never invents
  finished work. Settings + config. User-guide `17-sessions`,
  `05-configuration`.
- [x] **Killall mid-turn after first activity** — prior Quit→marker path still
  failed dogfood because `in_flight_prompt` is cleared on first server activity
  (tools/chunks/subagents). Whole-turn `cancel_resume_prompt_text` + process
  shutdown arm write the marker on first SIGTERM even when the event loop is
  slow. Report:
  `.agents/reports/impl-killall-resume-again-2026-08-08.md`.
- [x] **Killall e2e (eager marker)** — dogfood still saw no marker under real
  iso sessions: SIGTERM-only write races when client+leader share the binary
  name. Turn-start now **eagerly** persists `canceled_turn_resume.json` (fsync);
  session load re-queues + “Continuing interrupted turn...”. Report:
  `.agents/reports/impl-killall-resume-e2e-2026-08-08.md`.
- [x] **Ctrl+C closes plan approval** — empty-composer Ctrl+C while plan
  approval is open (soft-park or side panel) abandons the park (same as panel
  Quit). Non-empty clears draft first; second empty abandons. Unit tests in
  `agent_view/plan.rs`.
- [x] **Ctrl+C dismisses rewind overlay (2026-08-09)** — while the rewind turn
  picker / Esc-dismissible rewind phases are open, **Ctrl+C** maps to the same
  dismiss path as Esc (`RewindDismiss` / `DismissError`); does not quit the app
  on that press. Bare `c` on ModeSelect still means conversation-only; Ctrl+C
  does not. Aligns with L1 modal-free (no exclusive trap without a cancel
  chord). Report: [`.agents/reports/impl-ctrl-c-rewind-picker-2026-08-09.md`](.agents/reports/impl-ctrl-c-rewind-picker-2026-08-09.md).
- [x] **Rewind skips missing intermediate compaction checkpoints (2026-08-09)** —
  cross-compaction conversation rewind loads the latest covering checkpoint when
  an earlier marker file is missing/corrupt; fails soft only when the covering
  checkpoint for the target is unavailable. New compacts write the checkpoint
  **file before** the `updates.jsonl` marker. Report:
  [`.agents/reports/impl-rewind-compaction-checkpoint-missing-2026-08-09.md`](.agents/reports/impl-rewind-compaction-checkpoint-missing-2026-08-09.md).
- [x] **OAuth 403 `bad-credentials` → auth path (2026-08-09)** — HTTP 403 with
  body wording `unauthenticated:bad-credentials` / “OAuth2 access token could
  not be validated” classifies as **auth**, not free SuperGrok period limits and
  not bare policy 403: try one OIDC session refresh, then **ReAuthRequired** /
  `/login` UI (not yellow Internal error JSON). Policy / content-safety 403 and
  credit-exhausted 403 paths unchanged. Free SuperGrok period % chrome is
  unrelated to this failure. Report:
  [`.agents/reports/impl-oauth-403-bad-credentials-2026-08-09.md`](.agents/reports/impl-oauth-403-bad-credentials-2026-08-09.md).
  After re-auth, resend the failed message (product may offer re-auth stash
  auto-resubmit when the re-auth prompt is shown).
- [x] **Multi-track also-guard (first cut)** — `todo_write` accepts
  `meta.taskId` (subagent id); demoting `in_progress` → `pending` is rejected
  while that subagent is still Running (complete/cancel always ok; unbound
  demote still allowed). Prompt + tool description teach bind-after-spawn.
  Report: `.agents/reports/impl-ctrl-c-killall-resume-also-guard-2026-08-07.md`.
- [x] **Token Economy + resume in Settings GUI** — Settings modal covers
  economic mode, implement-effort caps (max/min/desired/lock), period pacing,
  local ledger / reconcile toggles, and resume-canceled-on-restart. Persist via
  config.toml. User-guide `05-configuration`.
- [x] **Plan mode selection + screenshots** — revise/clarify feedback carries
  `@plan.md:N` (or `N-M`) + quoted line text for single- and multi-line
  highlight; **Ctrl/Cmd+V** clipboard images and file-path paste attach on the
  plan composer (soft-park or side panel, Preview or Prompt); F9 / `/screenshot`
  can auto-attach when plan approval is open; images ride Interject with
  approve notes / revise / clarify on the same turn. User-guide `19-plan-mode`.
- [x] **btw Copy entire contents (B1)** — Done panel focused `y` + chrome `[y]`
  copies full plain text (`/btw <question>` + complete rendered answer, not
  viewport-only). Detail:
  [`doc/dev/research/btw-copy-followup-plan-trigger-2026-07-26.md`](doc/dev/research/btw-copy-followup-plan-trigger-2026-07-26.md)
- [x] **btw multi-turn follow-up (B2)** — same `btw_session_id`; shell injects
  prior Q/A; in-panel `[a]` composer; `full_copy_text` whole thread; history is
  multi-entry `btw_history.jsonl` (one `BtwEntry` per turn). Detail: same brief.
- [x] **Plan entry: incidental “plan” ≠ plan mode (B3)** — tighten
  `enter_plan_mode` tool description (explicit intent only); drop from auto
  name-allowlist; auto fast-path **PromptUser** even as Read. `/plan` + settings
  still user-initiated. Not a client keyword ban.
- [x] **Trailing-whitespace strip after product edits** — default ON for
  `search_replace`, OpenCode `write`/`edit`, `apply_patch`, hashline edit;
  shared `util/trailing_ws`; disable with `GROK_STRIP_TRAILING_WHITESPACE=0`.
  Report: [`doc/dev/research/trailing-ws-strip-2026-07-26.md`](doc/dev/research/trailing-ws-strip-2026-07-26.md)
- [x] **ASCII scrub of assistant output** — default ON at stream Text +
  chat_state + fallback chunks; map em/en dash, smart quotes, zero-width/NBSP
  → ASCII-safe; env `GROK_SCRUB_ASCII_PUNCT=0` + `[ui] scrub_ascii_punct` +
  Appearance settings row; agent disable only with permission approval
  (`disable_ascii_scrub` tool → permission UX; AllowOnce session / AllowAlways
  disk `[ui] scrub_ascii_punct` / Reject keeps on). Report:
  [`doc/dev/research/ascii-scrub-assistant-2026-07-26.md`](doc/dev/research/ascii-scrub-assistant-2026-07-26.md)

### Dogfood / next session handoff (2026-08-09)

**Read first (next task, recon, or dogfood):**

1. **Operator install gate** — code can be green while old TUIs still run a
   deleted-inode binary. Checklist:
   [`.agents/reports/d0-dogfood-checklist-2026-08-09.md`](.agents/reports/d0-dogfood-checklist-2026-08-09.md).
   Package status for Works A/B/C/E + prior plan fixes:
   [`.agents/reports/impl-remaining-plan-wave-2026-08-09.md`](.agents/reports/impl-remaining-plan-wave-2026-08-09.md).
2. **Shipped this wave (tree + reports; dogfood after install):** decisive plan
   **Revise**; same-batch `plan.md` write + `exit_plan_mode`; plan decision
   surface (**empty Enter never approves**; **Plan ready. Side panel open**);
   revise/clarify in-flight status + no idle CTA re-arm; caret empty half
   `text_primary`; OAuth 403 bad-credentials → refresh + re-auth UI; rewind
   missing intermediate checkpoints; Ctrl+C dismisses rewind; status
   `[pause]`/`[resume]` + red `[stop]`; soft stop chord-only; composer Enter
   `send`/`queue`/`interject` cue; compact `free SuperGrok period · N%` +
   footer `not the active spend path:`. Links live on the product bullets
   above (P1–P3 reports under plan / caret bullets).
3. **In flight / not shipped yet**
   - **Auto-resume after error terminal on rebuild/reopen** — expected operator
     contract if the last terminal was an error (not only cancel-resume marker).
     Document as shipped only when
     `.agents/reports/impl-rebuild-auto-resume-after-error-2026-08-09.md` (or
     equivalent) is green in tree. Distinct from continue interrupted turn
     (`canceled_turn_resume.json`). After any auto-resume, **403 bad-credentials
     may still need `/login`**.
   - **Soft-stop button** — not shipped; soft stop stays `Ctrl+Shift+S` only.
   - **Mid-sample freeze without cancel** — not shipped (global pause cancels
     turns; soft stop only stops queue drain after the current turn). Do not
     invent a media-player freeze metaphor.
4. **Still open (residual, not this install alone)**
   - Free SuperGrok period flat % / server C4 debit — paste-ready ticket
     [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md);
     multipoll addendum
     [`.agents/reports/c4-ticket-addendum-2026-08-08-multipoll.md`](.agents/reports/c4-ticket-addendum-2026-08-08-multipoll.md).
     Never invent free SuperGrok period used % on the client.
   - Agentic fmt/clippy ACP; thoughtful todo tracking process — [`RESIDUAL.md`](RESIDUAL.md)
     Open (plan after dogfood priority).
5. **Useful regression filters for this wave** (after recon / before dogfood
   claims):

```bash
cargo test -p xai-grok-pager --lib -- soft_park_revise_cta_click_submits_cancelled_immediately \
  panel_empty_prompt_s_submits_cancelled_immediately plan_panel_click_clarify_revise_quit \
  panel_prompt_empty_enter soft_park_present_status after_revise re_present_after_revise \
  plan_feedback_queue paint_composer_box_cursor_grapheme_phases_keep_letter \
  left_through_letters_empty_phase_not_neon
cargo test -p xai-grok-shell --lib -- split_tool_batch_before_exit_plan_mode \
  replay_skips_missing_intermediate_checkpoint
cargo test -p xai-grok-sampling-types --lib -- credentials_rejected forbidden_bad_credentials
cargo test -p xai-grok-sampler --lib -- api_403_bad_credentials classify_forbidden
cargo test -p xai-grok-pager --lib -- ctrl_c_dismisses_rewind enter_prompt_mode_matrix \
  work_control_chrome_matrix pause_button_click_dispatches_global_pause
```

Process law (plain English, no bad metaphors): host + project `AGENTS.md`
§ Prose + tone / hard constraint 4. Not re-dumped here.

### Skills (multi-source)

Skills are loaded from several places; the product on this branch owns the
machinery. Full map: `doc/dev/research/where-skills-come-from-2026-07-24.md`,
user-guide [`08-skills.md`](crates/codegen/xai-grok-pager/docs/user-guide/08-skills.md).

| Source | Role |
|--------|------|
| Project `.agents/skills`, `.grok/skills` | Git-trackable on the branch (supported; may be empty) |
| `~/.agents/skills` then `~/.grok/skills` | Host operator overlay (agents wins) |
| `[skills].paths` / server inject / plugins | Config and managed dirs |
| `~/.grok/bundled/skills` | Platform cache from network bundle sync |

**Process pins that must survive recon** (import / onto): document in **FORK +
AGENTS + product user-guide** when product-facing; **dual-pin** host skills
(`~/.agents`) when operator-only. Host skill git alone does not ride product
history. Chat-only pins die at compaction.

### What recon keeps / clobbers

| Path | Import | Put-history | Join (`-s ours`) |
|------|--------|-------------|------------------|
| Paths in `FORK_PATHS` (AGENTS, RESIDUAL, FORK, `docs/upstream-*`, join/hermetic/assert/`recon-status` scripts, `.grok/workflows`, `doc/dev`, …) | **Restored** from base; post-restore `assert-process-pins` | Via cherry-picks | Tip tree kept |
| Product commits after seed | N/A (tree = xAI + restore) | Cherry-picked onto tip | Tip tree kept |
| Paths **not** in `FORK_PATHS` and absent from xAI | **Dropped** | Only if stacked | Cannot backfill missing |
| Shared user-guide / crate seams | xAI base | Conflict resolve | Tip tree only |
| Host `~/.agents/skills`, `~/.grok/AGENTS.md` | Untouched | Untouched | Untouched |

Assert: `./scripts/assert-process-pins.sh` or `just upstream-assert-process-pins`.
Detail: `doc/dev/research/fork-paths-hardening-2026-07-24.md`,
`doc/dev/research/skills-survive-upstream-recon-2026-07-24.md`,
[`docs/upstream-history.md`](docs/upstream-history.md).

Novel Surmount crates use the **`grok-*`** prefix (example: `grok-rate-limit`).
Upstream crate paths stay **`xai-grok-*`** for mergeability.

### Upstream regression filters

**Process pins** survive import via `FORK_PATHS` restore +
`assert-process-pins` (path presence and light content sniffs). That gate does
**not** prove product behavior inside shared `xai-grok-*` crates.

**Product seams** (DOGE default + rails, titles-on / no `hide_title_bar`,
stuck-retry clear, dual-auth + multi SuperGrok + Heavy fresher-slot load,
console team prepaid gaps, plan soft-park side panel + three surfaces,
OpenRouter, Clear finished non-overlap / quiet paint, click tasks chrome open
subagent, Worked-for in-place, composer caret green + no residue, lower-left
throbber magenta, bubble copy + pointer cursor, …) live inside those crates.
They survive onto only through **cherry-picks / conflict resolve** and stay
honest through **cargo tests**. After recon, run the assert **and** the
product filter block (or at least `just check`).

Full filter catalog (why each exists + every residual Validate honesty block):
[`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md).
Open residual still points at the same commands under RESIDUAL § *Validate
honesty* (D0 can demote; the catalog is durable).

Operator cheat sheet (post-import / post-onto tip):

```bash
./scripts/assert-process-pins.sh
./scripts/assert-process-pins.sh HEAD   # or onto tip

# Core product harden (UI / DOGE / Human rail / titles / retry / shell collision)
cargo test -p xai-grok-shared --lib -- hide_header stale_hide_title
cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config doge_accent_user_is_pure_green doge_accent_system_is_pure_cyan
cargo test -p xai-grok-pager --lib -- user_prompt_block_accent user_prompt_prefix_matches recap_accent
cargo test -p xai-grok-pager --lib -- hide_header window_title titles_on_session default_title_items shell_collision retry_chrome_soft_reconnect
cargo test -p xai-grok-pager --test settings_e2e -- hide_header
# Plan soft-park three surfaces + Clear / click-open / Worked-for / caret / throbber
cargo test -p xai-grok-pager --lib -- exit_plan_mode_soft soft_park_draw clear_finished click_tasks_model_timer \
  parked_marker_not_stacked paint_composer_box_cursor_uses_human caret_move_clears \
  doge_idle_subagent_still_running doge_tool_running_spinner
cargo test -p xai-grok-shell --lib -- stream_started_emits_retry_state_stream_resumed
cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason retry_footer_backoff stream_headers_timeout_defaults
cargo test -p xai-grok-sampler --test stream_headers_timeout

# Dual-auth / multi SuperGrok / Heavy fresher-slot load / console prepaid
cargo test -p xai-grok-shell --lib -- load_candidates_prefers_live resolve_auto_uses_live_supergrok dual_supergrok upsert_personal_then_business
cargo test -p xai-grok-pager --lib -- show_limits format_supergrok_session footer_names_live_principal

# Plan soft-park side panel + bubble copy / pointer
cargo test -p xai-grok-pager --lib -- exit_plan_mode_soft plan_panel_preview_ctrl_v soft_park_prompt_ctrl_v bubble_copy_ pointer_cursor

just check   # full gate before push/PR
```

## CI and local quality

**CI is for checks only** — never build a shippable release package in GitHub
Actions (supply-chain boundary). Humans package from a trusted tree when ready.

| Command | Role |
|---------|------|
| **`just check`** or **`just ci`** | Full local gate (flake-meta + prep + fmt/clippy/tests) — **run before push** |
| **`just test`** | Quality suite without re-running full flake prep |
| **`just build` / install** | Optional release-style package (not CI) |

GHA quality job: flake-meta → ci-prep → `just test` (see `.github/workflows/ci.yml`).
There is **no** `ci-quick` or `ci-host` recipe.

**PATH hermeticity (CI / low-mem):** with `CI_LOW_MEM=1`, `cargo-ci` enters
`nix develop .#ci`, then `scripts/with-ci-hermetic-path.sh` rebuilds `PATH`
from **`/nix/store` bins only** (ci-tools + stdenv: rustc, nextest, mold, git,
python3, coreutils, …). Host desktop tools (`pw-record` / `parec` / `arecord`,
…) are not visible to quality tests — matches headless GHA. Interactive
`just dev` / default shell keep impure host `PATH`. Audio recorders are
intentionally **not** in `ci-tools`; `python3` **is** (cgroup + mock LSP e2e
spawn it under scrubbed PATH). Escape hatch: `GROK_CI_ALLOW_HOST_PATH=1`.
Closest GHA repro: `CI_LOW_MEM=1 CI_SYSTEM=x86_64-linux just ci`.

## Versioning and “am I up to date?”

| Idea | Practice |
|------|----------|
| **Upstream owns the package version number** | Keep lockstep with the upstream tree we track (`CARGO_PKG_VERSION`) |
| **Our identity is the git revision** | Binary shows **upstream version + short git SHA** |
| **No second release train** | No Surmount stable/alpha channel mirroring SpaceXAI |
| **No default xAI auto-update** | Would advertise official `grok` builds |

Illustrative only (not necessarily this checkout):

```text
grok-oss <upstream-version> (<short-sha>)
```

```bash
grok-oss --version
grok-oss update --check          # vs github.com/SurmountSystems/grok-oss main
grok-oss update --check --json
```

`SOURCE_REV` at the repo root is a **monorepo export pin** (full upstream-side
SHA recorded for the tree we absorbed), not a substitute for “what is HEAD.”

If behind: from a checkout run **`/rebuild`** (TUI) or **`grok-oss rebuild`**
(CLI), or rebuild/reinstall by hand — not the official
`curl https://x.ai/cli/install.sh` path (that installs upstream **`grok`**).

## Multi-session rate limits

Concurrent `grok-oss` processes share cooldowns under `~/.grok/rate_limits/`
(`grok-rate-limit`). On HTTP 429-style limits, the strictest wait wins across
processes. Disable shared coordination with `GROK_DISABLE_SHARED_RATE_LIMIT=1`.

Product HTTP paths that wait before send and observe on 429 (403 only when a
retry hint such as `Retry-After` is present):

| Class | Provider key shape | Examples |
|-------|--------------------|----------|
| Chat / inference | host + key fingerprint | sampler (xAI, SuperGrok proxy, OpenRouter, BYOK base URLs) |
| SuperGrok billing | proxy host + session fingerprint | `GET …/billing?format=credits`, auto-topup |
| Management API | management host + management-key fingerprint | prepaid, postpaid, usage series, key validation |
| Imagine image | host + fingerprint + `imagine` | `image_gen`, `image_edit` |
| Imagine video | host + fingerprint + `video` | `video_gen` start + poll |
| Voice STT | host + fingerprint + `voice` | streaming `wss://…/v1/stt` |
| Responses | host + fingerprint + `responses` | `web_search` |
| GitHub | logical `github` | OSS update compare |

Waits prefer server headers (`Retry-After`, then `x-ratelimit-reset`) over
hardcoded tier tables. Public docs (accessed 2026-08-03):

- [xAI rate limits](https://docs.x.ai/developers/rate-limits) (per-model RPS/TPM;
  Imagine image/video have separate RPS; Voice/Imagine tier increases via sales)
- [OpenRouter limits](https://openrouter.ai/docs/api_reference/limits) (honor
  `Retry-After` / `X-RateLimit-*` on 429)
- [GitHub REST rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
  (primary + secondary; `Retry-After` / `x-ratelimit-reset`)

## Canonical repo

<https://github.com/SurmountSystems/grok-oss>

## License

Apache License 2.0 — [`LICENSE`](LICENSE).
Third-party: [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES).
