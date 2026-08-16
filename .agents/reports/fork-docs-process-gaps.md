# Process-tool gaps that let a chrome-only 1.0.3 land pass

**Role:** how standing law and the existing land tools failed to catch crate-seam loss after the Grok Build 1.0.3 restack. Not a second inventory. Not a product restore.

SuperGrok is a **paid** product. This report says **included SuperGrok period limits**, SuperGrok dollar credits, and console team prepaid / console API credits as distinct meters.

## 1. What law already says vs what tools enforce

| Claim (AGENTS / FORK / git-recon / catalog) | What actually fails closed |
|---------------------------------------------|----------------------------|
| Import restores only `FORK_PATHS` (docs, scripts, packaging, `grok-rate-limit`, workflows). Product seams inside `xai-grok-*` are not path-restored. | `scripts/import-upstream-export.sh` restores that list, then runs assert. Post-restore NOTE still names only OpenRouter, binary rename, and sampler. It does not name the seven land classes. |
| Assert proves files exist. It does not prove crate contracts. | `scripts/assert-process-pins.sh` checks required files/dirs. Worktree-only sniffs: AGENTS "parent is coordinator", FORK upstream words, README Grok OSS, FORK "non-excepted Python", user-guide `08-skills.md` "not a Python runtime", and junk `.py` under product skill roots. Tree-ish mode does not run those sniffs. No catalog `fn` names. No cargo. |
| Land must cover **seven** inventory classes. Chrome-only is a failed land. `just check` cannot fail a deleted catalog test. | `just check` / `just ci` run whatever tests remain in the tree. A deleted catalog test is invisible. There is no `just` land recipe that requires the catalog cheat sheet. |
| After assert, `rg` each catalog identifier for a matching `fn`. Helper-green is a failed land. | This is **agent procedure**. No script fails if `sampling_config_auto_use_*` or `show_spend_ingests_*` is missing. |
| Host `git-recon` `recon:land` must run assert + catalog + helper-green ban + paint/dogfood. | Skill still says **six** inventory classes and omits class 7 (product skills are not a Python runtime). Agent depth in the skill is still "L2 must spawn L3 on many greps." Project `AGENTS.md` says three layers **always**. |

**FORK numbering is itself a trap.** Under *Land checklist*, numbered items 1–4 are steps (assert, cargo, `rg` names, helper-green). The following "seven inventory classes" then mix two process notes (item 5 is `FORK_PATHS` is docs-only; item 6 is "must not be chrome-only") with the product classes. Dual-auth hop and last-session are buried inside item 6. The durable catalog and the FORK cheat-sheet cargo blocks already list the real seven product classes in order: (1) CLI identity, (2) config is a surface, (3) `/spend` ingest, (4) DOGE/chrome paint, (5) dual-auth hop after included SuperGrok period limits are full, (6) last-session on start, (7) skills are not a Python runtime.

Law already forbids treating helper-green as proof (`contains("grok")` on `--version`, theme file exists, schema v1 exists, serde `hide_header`, rank helpers without hop keys, bundle still has `memory.py`). That ban is prose. It is not a gate.

## 2. Why a chrome-only 1.0.3 land could pass

1. **Cherry-pick plus compile mop looked green.** Onto replayed product commits onto `e5fd4816`. Conflict resolve and a lib compile mop kept DOGE files, some rails helpers, and schema v1. That is exactly what `just check` rewards: remaining tests pass; deleted tests do not run.
2. **Assert was satisfied.** `FORK_PATHS` docs and scripts were on the tip. Join (`-s ours`) keeps that tip tree. Assert cannot see that `sampling_config` hop keys were empty, `/spend` wrote `DoubleEntryReport::default()`, `/settings` had leftover unread rows, or Welcome said Grok Build.
3. **Agents treated paint as the land.** Residual Open says the first 1.0.3 inventory was chrome-only. The host skill still frames land as "assert + six classes + paint." Paint is class 4. Dual-auth hop, `/spend` ingest, unread config, first-token `grok-oss`, last-session, and skills-not-Python are the other classes. Screenshots of rails and five CTAs do not prove hop keys or ledger ingest.
4. **Helper-green hid CLI and config loss.** `grok 1.0.3` matches substring `grok`. Serde default tests stay green when `/settings` has no row and nothing reads the field. Rank helpers stay green when `sampling_config_for_model` does not fill console failover after included SuperGrok period limits are full.
5. **User-guide is not in `FORK_PATHS`.** Onto conflict can drop `/limits`, `grok-oss`, DOGE default, and last-session copy. Catalog already says a guide with zero `/limits` hits is a failed land. Nothing in assert or CI runs that `rg`.
6. **Catalog is D3 + FORK pointer, not a mechanical land job.** Import does not restore crate tests. Onto only keeps tests that cherry-picks kept. No step fails "FORK land names no longer appear in the catalog" or "catalog names have no `fn`."

The 2026-08-13 restore wave (hop, unread `/settings`, `/spend` ingest, leftover plan chrome) is evidence the first land report was incomplete, not that the law was missing.

## 3. Improvements that reuse FORK + catalog + git-recon (no new board)

Keep **one** inventory: `FORK.md` § *Land checklist* plus [`doc/dev/upstream-regression-filters.md`](../../doc/dev/upstream-regression-filters.md). Do not invent a second list.

1. **Renumber FORK's seven classes to match the catalog.** Steps stay 1–4 (assert, cargo, `rg` `fn`, helper-green). Classes become the seven product rows already in the catalog cheat sheet. Move the `FORK_PATHS` and chrome-only sentences back to the intro. Dual-pin the same seven names into host `git-recon` (replace every "six-class").
2. **Small assert extension, not a new inventory.** After files exist, fail if `doc/dev/upstream-regression-filters.md` is missing (it lives under `doc/dev`, which is only "dir non-empty" today). Fail if FORK land-section identifiers (the seven class titles plus a short pinned name list already printed in the FORK cheat sheet) do not appear in the catalog. Optionally `rg` a **fixed** list of catalog identifiers for `fn ` in `crates/` (same names the cheat sheet already prints). Missing `fn` = land failed. Do not scrape FORK for every historical neighbor test. Do not add a second table of tests.
3. **`just upstream-land-filters` (or equivalent) wraps the existing cheat sheet.** Recipe runs assert, then the catalog operator cheat-sheet cargo blocks, then prints "name check before cargo." Land agents call this instead of inventing a chrome subset. `just check` stays the quality gate and still cannot replace missing `fn` names.
4. **Import post-restore NOTE prints the seven classes and the catalog path.** Same sentences FORK already uses. No new checklist file.
5. **git-recon `recon:land` must refuse chrome-only closeout.** Skill closeout requires the seven-class recipe (or the new just alias) plus `rg` name check. Paint/dogfood stays an operator check **after** those `fn`s exist. Dual-pin always-three-layer here (L2 always spawns L3 for land cargo/`rg`). Do not grow project `AGENTS.md`.
6. **User-guide land sniff stays a one-liner.** Catalog already: zero `/limits` hits is a failed land. Assert worktree sniff can require `/limits` and `grok-oss` in the shared guide without copying the guide into `FORK_PATHS`.

## 4. Dual-pin: host skill vs project docs

| What | Where | Do not |
|------|--------|--------|
| Land recipe, seven class names, helper-green ban, `rg` `fn` | `FORK.md` (short) + catalog (commands) + host `git-recon` `recon:land` | A novel in D1 `AGENTS.md` |
| Survive-recon pointer (assert + seams need cargo + chrome-only fails) | Project `AGENTS.md` § *Survive recon* (already there, keep short) | Repeating the cheat-sheet cargo blocks in AGENTS |
| HITL runbook / import review checkbox | `docs/upstream-history.md` (D2) | Recon diaries in AGENTS |
| Operator mid-stack continue / unsigned recon exception | Host skill only, with a one-line FORK pointer | Copying the unsigned table into AGENTS |
| Process corrections that must survive import | Branch (`FORK` / AGENTS pointer / `docs/upstream-*` are in `FORK_PATHS`) **and** host skill same turn | Host-only pin (dies for agents without overlay) or AGENTS-only novel |

Host `~/.agents/skills` and `~/.grok/AGENTS.md` are outside product git. Import never touches them. Onto never cherry-picks them. That is why land law that recon agents actually run must live on the branch **and** in the skill.

## 5. Process pins (docs) vs product-testable contracts

**Docs only (cannot cargo-test the behavior of the agent):** always-three-layer depth; parent is HITL coordinator only; never agent `git commit`; no bulk replace; complete plan verticals; session-board closeout; "also" is additive. Assert can only sniff that AGENTS still contains the coordinator sentence. A restack that keeps the file and weakens the skill ("spawn L3 when many greps") will not fail cargo.

**Product-testable (named cargo tests in the catalog):** first token `grok-oss`; `/settings` rows plus runtime readers; `/spend` ingest of `usage.jsonl` plus `reconciliation_run`; paint (rails, caret, compact included SuperGrok period limits meter, titled composer frame, five CTAs); `sampling_config` hop after included SuperGrok period limits are full (rank helpers are not this); last-session on start; sanitize/intercept so product skills are not a Python runtime; user-guide sentence in `08-skills.md`.

Do not try to encode always-three-layer as a land class. Do not treat assert file presence as a substitute for the cargo classes.

## 6. User-guide fork pins: FORK one-liners + link, not a copy

The shared guide is **not** in `FORK_PATHS`. Onto must conflict-resolve these. FORK should keep **one line each** and link the page. Do not paste the pages into FORK or AGENTS.

| Guide page | Fork-specific pin (one line) |
|------------|------------------------------|
| `01-getting-started` | Binary is `grok-oss`. Bare interactive open is last session for this cwd, not Welcome. |
| `02-authentication` | SuperGrok is paid. Distinct meters. `/limits` and compact chip. Hop after included SuperGrok period limits are full. |
| `03-keyboard-shortcuts` | Plan keys and Enter cue (send / queue / interject). Empty Enter never approves a plan. |
| `05-configuration` | `hide_header` is in-app only. Titles use `title.enabled`. `[subagents] allow_worktree` defaults false. Token Economy table. |
| `06-theming` | Default theme is DOGE. Human green / agent magenta roles. |
| `08-skills` | Product skills are not a Python runtime (allowlisted CLI stubs + office/docx/pptx/xlsx/pdf only). |
| `16-subagents` | Worktree isolation off by default. Soft interject never cancels. |
| `17-sessions` | Last-session on start vs `-c` / `--resume` vs `canceled_turn_resume.json`. Resume examples use `grok-oss`. |
| `19-plan-mode` | Present is not Approve. Five CTAs. Empty Enter never approves. Freeform questions, not the questionnaire modal. |
| `22-permissions-and-safety` | Always-approve is tool permissions only, not plan Approve. |
| `24-monitoring-usage` | `/spend` ledger vs org metrics. Do not mash meters. |

Catalog already owns `user_guide_resume_and_version_examples_use_grok_oss` and `user_guide_skills_are_not_a_python_runtime`. A `/limits` hit-count sniff belongs next to those, not as a new inventory.

## 7. Residual Open: honesty leftovers vs already shipped

Read D0 Open only. Do not re-open closed campaign writeups.

**Honesty leftovers (still true, not a second land class):**
- Live TUI can remain the old 1.0.3 binary until a successful rebuild/install. Source restore is not dogfood.
- Host `~/.grok/docs` extract stays stale until the next product launch.
- `sampling_identity` column exists and ingest leaves it unused.
- Dogfood / next-session gate stays open until install + quit old TUIs + reopen.
- Plan `plan.md` bodies can still invent freeform approve menus (product chrome does not).
- Soft remainders: auto-bind every Task, sticky multi-track toast, archive browser, richer screenshot fonts, live-rule stream feedback (parked).

**Already shipped (do not re-inventory as open land work):**
- Dual-auth hop after included SuperGrok period limits are full, unread `/settings`, `/spend` ingest: restored in source (2026-08-13 reports). Prove with catalog classes 2, 3, 5, not a new board.
- Five-CTA plan panel, last-session tests in tree, nucleo reuse-per-root, Rust 1.97.1 pin, UDAX T0–T6, ASCII scrub, hide_header vs titles, DOGE default, multi-track first cut (`meta.taskId`), fib leaves, soft interject, stuck-retry/`StreamResumed`.
- Process features to **plan later**, not this land mop: agentic fmt/clippy via ACP (`feat:agentic-fmt-clippy-acp`); thoughtful todo tracking (`feat:thoughtful-todo-tracking-process`); structured token-efficient conversation (`plan:structured-token-efficient-convo`); Sapient Experience product steers (parked).

C4 (server included SuperGrok period debit) remains an xAI ticket. It is not a restack land class.

## Do / Do not change

**Do**
- Align FORK class numbering with the catalog's seven product classes.
- Dual-pin host `git-recon` from six classes to seven, and from "L3 when many greps" to always-three-layer. Keep the skill short.
- Extend `assert-process-pins.sh` so the catalog file exists and FORK land names still appear in that catalog. Optionally `rg` the cheat-sheet identifiers for `fn `.
- Add a just alias that runs the **existing** catalog cheat sheet after assert.
- Print the seven classes on import post-restore.
- Add a user-guide `/limits` + `grok-oss` sniff. Keep the guide out of wholesale `FORK_PATHS`.
- Keep D1 `AGENTS.md` Survive recon as a pointer.

**Do not**
- Invent a second inventory, board, or residual land class list.
- Dump the cheat-sheet cargo novel into `AGENTS.md`.
- Treat `just check` as land proof.
- Put the whole user-guide in `FORK_PATHS`.
- Encode always-three-layer as a cargo land class.
- Call assert a contract proof.
- Close land on paint screenshots or helper-green.
- Re-open shipped residual as if the 1.0.3 crate seams were never restored in source.
- Call SuperGrok free, or mash included SuperGrok period limits with SuperGrok dollar credits or console team prepaid / console API credits.
