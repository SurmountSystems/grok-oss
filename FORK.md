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
- [x] **SuperGrok OAuth ↔ console API key dual-auth** — first-party resolve merge (session primary + console failover by default; `preferred_method=api_key` reverses); identity hop on **credit** and **plain 429** (session→key clears bearer; key→session via JWT in failover list); dual-host hop; **D3** process-local credit exhausted-fingerprint memo + distinct hop status/toast (credit vs rate-limited; labels only); rate-limit hop uses temporary shared `grok-rate-limit` cooldown (not credit memo); kill-switch clears key failover; console keys in keyring/`provider_credentials.json` + env/auth.json; **live re-bind without prior stash** (`session_bearer_resolver`); **multi-add** `grok login --api-key` + `--list-api-keys` (fingerprints only). Plans: [`.agents/plans/plan-secure-key-failover.md`](.agents/plans/plan-secure-key-failover.md), [`.agents/plans/plan-rate-limit-failover.md`](.agents/plans/plan-rate-limit-failover.md). Residual: dual OAuth S3 out of scope; optional durable memo.
- [x] **Economic mode** — soft-cap effective context at the Grok 4.5 long-context price cliff (~200k); `/economic-mode`; settings default on
- [x] **Auto-compact default 95% + live-apply** — stock Grok 4.5 catalog omits a per-model undercut (was 80); remote `models_cache` undercuts on stock models are dropped so the product default applies; user session/env still win; banner shows usage **and** configured threshold. Settings commit live-applies to open sessions (`restart_required: false`): disk persist → ACP `x.ai/auto_compact_threshold_changed` → `SessionCommand::SetAutoCompactThreshold` → CompactionConfig Cells (same write path as model switch). Live-apply pushes the **committed Settings value** (race-safe vs disk); env `GROK_AUTO_COMPACT_THRESHOLD_*` wins again on the next full resolve (spawn / model switch). Detail: `docs/dev/research/rca-auto-compact-early-fire.md`
- [x] **Auto-run `/implement`** — after a successful turn, queue a follow-up implement block when present; **appends** after any already-queued prompts (does not drop them); economic mode can clamp implement `--effort`
- [x] **Shared rate limits** — crate `grok-rate-limit` (Surmount name, not `xai-`); cooldowns under `~/.grok/rate_limits/`; optional `GROK_DISABLE_SHARED_RATE_LIMIT=1`
- [x] **Updates** — no xAI auto-update channel by default (wrong product). `grok-oss update --check` compares to Surmount `main`. Escape hatch: `GROK_OSS_ENABLE_XAI_UPDATER=1`
- [x] **Soft interject only** — mid-turn interject (Ctrl+Enter / terminal alts, queue `[Interject]`) injects into the **current** turn and **never cancels**. Cancel is Esc/stop only. Shell contracts: `interject_contract_*` tests. Do **not** re-unify user mid-turn steer on `SendPromptNow` (cancel-and-send). Idle + live background subagents holding the queue: status `… Interject to force`, queue row `[Interject]` force-drains (same as chord). User copy: tip/status say **Enter to interject** (not “send now”). Esc on cancel-turn panel dismisses only. **Parked sendable-wait exception (intentional):** while the agent is **blocked waiting** (task/subagent) **and the queue is empty**, plain Enter with text may still cancel-and-send to unblock immediately — not soft Interject; documented in user-guide `03-keyboard-shortcuts`. Detail: user-guide `03-keyboard-shortcuts` § during an active turn.
- [x] **Todo board survives auto-compact** — pager no longer clears the UI todo list on `AutoCompactCompleted` (Resources still held the board; UI wipe was a lie). Contract: `auto_compact_completed_preserves_todo_board`.
- [x] **plan.json honesty + resume board** — compact writes the **live** Resources `TodoState` to `plan.json` (no empty wipe). Resume loads `plan_state` again and re-emits ACP `Plan` from Resources / `plan.json` fallback (`RestoreTodoBoard`). Real SoT: in-memory Resources + on-disk **`resources_state.json`** (bridge path is named `tool_state.json` but registry rewrites to sibling `resources_state.json`); `plan.json` is a mirror + resume fallback. User-guide `17-sessions` documents both.
- [x] **Auto-seed user asks as todos** — real user turns seed protected `ask:<prompt_id>` (cap 20, truncated content); `ask:` is keep-unless-mentioned on `merge: false`. Helpers + tests in `xai-grok-tools` todo module.
- [x] **Default agent uses the todo board** — base `prompt.md` teaches `todo_write` (Planning section, gated on plan tool): multi-step / `feat:` / `bug:` / merge upsert / protected prefixes / red/green TDD for user-reported bugs & features / mark complete / Ctrl+T board. First empty→non-empty Plan auto-opens the todo pane once. Fork/copy includes `resources_state.json` (not only `tool_state.json`).
- [x] **Plan approval CTAs** — primary bar `a` approve · `A` approve w/ comment · `?` clarify · `s` revise · `q` quit (no primary Comment). Wire outcomes: approved / approved+notes / `"questions"` / cancelled / abandoned. Clarify keeps plan Active (answer-only; agent re-`exit_plan_mode`). User-guide `19-plan-mode`.

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
- [x] **Parent = HITL only** — main thread goals/spawn/join notes/human git; research + implementation in subagents. Hard stop on CI / multi-file. See [`AGENTS.md`](AGENTS.md)
- [x] **Subagent worktree policy** — prefer isolation none; product default
  `[subagents] allow_worktree = false` (empty config force-none; opt in with
  `true`). Spawn still forces none when false. User-guide migration notes in
  `05-configuration` + `16-subagents`. Host skills dual-pin todo namespaces
  (`plan:*` / `impl:*` / …) + worktree optional. Campaign:
  `doc/dev/campaigns/operator-orchestration-2026-07.md`
- [x] **`/execute-plan` honors `allow_worktree`** — host skill defaults to
  shared-cwd protocol (serial/disjoint writers, on-disk reviews, no worktree
  path handoffs); worktree only when policy allows; fall back if spawn forces
  none or create fails. Join:
  `doc/dev/research/execute-plan-no-worktree-2026-07-24.md`
- [x] **Todo levels product surface** — `todo_write` accepts optional
  `priority` + `meta` (`kind`, `parentId`, `namespace`); `merge: false`
  keep-unless-mentioned for protected prefixes (`plan:`, `impl:`, `pr-`,
  `recon:`, `residual:`, `ask:`, `feat:`, `bug:`). Feature suggestions use
  `feat:`; user-reported bugs use `bug:` (session board; not durable residual
  unless campaign-ranked). Red/green TDD for user-reported bugs/features.
  Light `[kind]` badge in todo pane. Join:
  `doc/dev/research/todo-levels-product-2026-07-24.md`
- [x] **Todo fib leaves + weighted progress** — optional `size` on items
  (only **1|2**; `meta.size` fallback); reject size on parents with children;
  progress = Σ leaf sizes (legacy item counts when no sizes); badge
  `N/M pts` in points mode; tool output `progress` + merge:false archive
  warning; prompt + tool blurb teach merge-only + fib structure. Join:
  `doc/dev/research/todo-progress-fib-2026-07-26.md`
- [x] **Cleared todo archive** — items dropped by `merge: false` (unprotected
  unmentioned) or ask-cap prune land on a capped `cleared_todos` ring on
  `TodoState` (max 200; Resources serde). Active board / Plan / todo pane stay
  active-only. Detail:
  `doc/dev/research/cleared-todos-archive-2026-07-25.md`
- [x] **Session notes channel** — `/note` stores operator mid-session
  annotations that are **not** pending main-turn prompts (session-local
  store; list via bare `/note` / `/notes`; count on `/tasks`). Does not
  replace on-disk L2 join notes. Join:
  `doc/dev/research/notes-channel-2026-07-24.md`
- [x] **Git recon depth** — host skill `/git-recon` (status → route →
  conflict ≤3 buckets → stage → human-sign → land; never agent-commit);
  product `scripts/recon-status.sh` + `just recon-status` (read-only probe);
  pin in `FORK_PATHS` + `assert-process-pins`; optional
  `.grok/workflows/git-recon-status.rhai`. Joins:
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
- [x] **Hide header + DOGE theme** — `[ui] hide_header` zeros top agent status
  bar, welcome location top bar, and dashboard location header; theme `doge`
  (display “DOGE”, no aliases): pure black/white + 8 pure primaries. Docs:
  user-guide `06-theming`.
- [x] **DOGE pure 8-colour palette** — durable pure palette (`#000000`…
  `#FFFFFF` + eight primaries) as product truth for `doge`; hard-threshold
  quantise + optional Floyd–Steinberg helper in
  `xai-grok-pager-render::theme::doge`; user-guide DOGE section. Project note
  (not an ECMA standard):
  [`doc/dev/specs/doge-pure-8-colour-2026-07-26.md`](doc/dev/specs/doge-pure-8-colour-2026-07-26.md).
- [x] **DOGE polish (Wave 2)** — context-bar **solid DOGE steps** (no mid-gray
  lerp); pure-primary **`doge.tmTheme`** for DOGE syntax; `hide_header`
  extended to welcome + dashboard headers.
- [x] **btw Done-panel keys in user-guide** — focused `y` copy full thread, `a`
  follow-up same session, Esc dismiss (`04-slash-commands`).
- [x] **Plan approval soft park (option A)** — `exit_plan_mode` parks durable
  approval with status chrome + toast; no hard modal takeover; modal on demand
  (`/view-plan`, status click, `ShowPlan` / reopen). Four CTAs + clarify RO
  unchanged. **B/C/D parked** (side panel / inline / config) unless A jars —
  do not invent. Design:
  [`doc/dev/research/plan-modal-softer-park-2026-07-26.md`](doc/dev/research/plan-modal-softer-park-2026-07-26.md)
- [x] **Plan mode selection + screenshots (P1–P4)** — revise/clarify feedback
  carries `@plan.md:N` (or `N-M`) + quoted line text for single- and multi-line
  highlight; paste screenshots on the plan prompt — images ride Interject with
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
  Join: [`doc/dev/research/trailing-ws-strip-2026-07-26.md`](doc/dev/research/trailing-ws-strip-2026-07-26.md)
- [x] **ASCII scrub of assistant output** — default ON at stream Text +
  chat_state + fallback chunks; map em/en dash, smart quotes, zero-width/NBSP
  → ASCII-safe; env `GROK_SCRUB_ASCII_PUNCT=0` + `[ui] scrub_ascii_punct` +
  Appearance settings row; agent disable only with permission approval
  (`disable_ascii_scrub` tool → permission UX; AllowOnce session / AllowAlways
  disk `[ui] scrub_ascii_punct` / Reject keeps on). Join:
  [`doc/dev/research/ascii-scrub-assistant-2026-07-26.md`](doc/dev/research/ascii-scrub-assistant-2026-07-26.md)

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

If behind: rebuild or reinstall from this repo / packaging — not the official
`curl https://x.ai/cli/install.sh` path (that installs upstream **`grok`**).

## Multi-session rate limits

Concurrent `grok-oss` processes share cooldowns under `~/.grok/rate_limits/`
(`grok-rate-limit`). On HTTP 429-style limits, the strictest wait wins across
processes. Disable shared coordination with `GROK_DISABLE_SHARED_RATE_LIMIT=1`.

## Canonical repo

<https://github.com/SurmountSystems/grok-oss>

## License

Apache License 2.0 — [`LICENSE`](LICENSE).  
Third-party: [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES).
