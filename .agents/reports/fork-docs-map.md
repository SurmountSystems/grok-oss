# FORK.md map for the next upstream-merge defense inventory

Source: `/home/hunter/Projects/surmount/grok-build/FORK.md` (771 lines, read in full on 2026-08-15). This is a map for a later writer. **Do not treat a `[x]` checkbox as proof the seam is in the tree.** Mark below as “FORK claims X; verify in code later.”

Style already in FORK (lines 69–70): *“Hierarchical: one line here → code or a linked doc for detail. Update this list when you ship fork work.”* Land checklist (547–558) repeats: named cargo filters, `rg` for a matching `fn`, helper-green is a failed land, screenshots are not the only check.

---

## 1. Section list (document order)

| Lines | Section | What it does |
|-------|---------|--------------|
| 1–13 | Title + affiliation | Unofficial fork of xai-org/grok-build; Apache-2.0; Surmount; not endorsed by xAI. |
| 15–24 | Vision | Faithful / complete history / open / distinct (`grok-oss`) / compatible `~/.grok` / superset. |
| 26–32 | Git flow | Feature branches → PR → `main`. Tool branches land via PR. Open PRs merge, not rebase. Links `docs/git-workflow.md`. |
| 34–40 | Remotes | `xai-org` vs `origin` (SurmountSystems/grok-oss). |
| 42–65 | Syncing with xAI | Import vs put-history vs join (`-s ours`). Never reset `main` to xAI. Links `docs/upstream-history.md`, import/onto logs. |
| 67–70 | Divergence inventory intro | Hierarchical one-liner rule + “update this list when you ship.” |
| 72–98 | ### Product | Large shipped-claim list: UDAX, ULID, usage.jsonl, last-session, branding, OpenRouter, dual-auth, meters, Token Economy, `/rebuild`, interject, plan CTAs, etc. |
| 100–106 | ### Packaging and build | AUR, Nix, **Rust 1.97.1**, justfile, release-dist debug sidecar. |
| 108–449 | ### Process | Process docs, onto, **always three layers**, todos, skills-as-Rust, then a long chrome dump (DOGE, rails, Clear finished, pause/stop, plan P1/P2, caret, bubble copy, …). Many product seams live here, not under Product. |
| 451–507 | ### Dogfood / next session handoff (2026-08-09) | Install gate, shipped-that-wave, not-shipped, residual C4, a short cargo filter block for that wave only. |
| 510–526 | ### Skills (multi-source) | Load order table + dual-pin. No named cargo tests in this subsection. |
| 528–545 | ### What recon keeps / clobbers | `FORK_PATHS` restore vs cherry-pick vs join. Assert proves files exist, not contracts. |
| 547–567 | ### Land checklist | Five land steps + **seven inventory classes**. Strongest defense text in the file. |
| 569–679 | ### Upstream regression filters | Seam list + operator cheat sheet with named `cargo test` filters. Points at `doc/dev/upstream-regression-filters.md`. |
| 681–703 | CI and local quality | Checks only; `just check` / `just ci`; no release package in GHA; PATH hermeticity. |
| 705–731 | Versioning | Upstream owns `CARGO_PKG_VERSION`; identity is git SHA; `/rebuild` not x.ai install.sh. |
| 733–761 | Multi-session rate limits | Shared `grok-rate-limit`; provider classes; public doc links (accessed 2026-08-03). |
| 763–770 | Canonical repo + License | GitHub URL; Apache-2.0. |

---

## 2. What is already inventoried

### Named tests on Product bullets (rare)

- Last session (77): `materialize_new_auto_opens_last_session_when_one_exists`.
- Binary / branding (78): `product_cli_name_is_grok_oss`, `product_version_line_uses_grok_oss_not_bare_grok`, `version_without_tty`, `print_exit_resume_hint_writes_expected_lines`, `user_guide_resume_and_version_examples_use_grok_oss`.
- Soft interject (92): `interject_contract_*` (prefix only).
- Todo board survives auto-compact (93): `auto_compact_completed_preserves_todo_board`.
- Ctrl+C plan (387): “Unit tests in `agent_view/plan.rs`” (no `fn` names).

Most other `[x]` bullets cite research notes or `.agents/reports/*`, not catalog `fn` names.

### Land-checklist + cheat-sheet inventory (547–667)

Already names the seven classes, helper-green bans, and a runnable filter block. Extra neighbor block (669–676): window titles, `shell_collision`, retry chrome, `exit_plan_mode_soft`, stream headers timeout, `failed_install_must_not_replace_or_signal_peers`.

Cheat-sheet filters that already exist in FORK (copy for the writer; verify `fn` later):

1. CLI identity (612–615): branding tests above + `resume_session_command_uses_grok_oss`.
2. Config surface (618–624): settings_e2e `hide_header` / `always_expand_thinking` / `scrub_ascii_punct` / `allow_worktree` / `bubble_copy_buttons` / `plan_approval_park`; `theme_choices_include_doge_and_default_is_doge`; `hide_header_zeroes`; `bubble_copy_buttons_on`; `prime_applies_scrub_ascii_punct_from_ui`; `resolve_subagents_copies_allow_worktree`.
3. Ledger `/spend` (627–628): `spend_path_ingests_usage_jsonl_and_records_reconciliation`; `show_spend_ingests_usage_jsonl_and_is_not_empty_default`.
4. DOGE / chrome (631–640): `default_theme_is_doge`, `resolve_from_config_no_config`, `doge_accent_user_is_pure_green`, `doge_accent_system_is_pure_cyan`, green rail + caret, magenta agent rail, compact included SuperGrok period limits meter, titled composer frame, five-CTA footer, auto-compact todo board.
5. Dual-auth hop (643–656): `sampling_config_auto_use`, sibling-included-before-extras, afterburner, combined remaining, Business/Team before personal, `limits_snapshot_*` flock, billing hub, compact meter + `active_spend_driver` while sibling has remaining, dual `/limits` JSON honesty.
6. Last-session (659): one test name.
7. Skills not a Python runtime (662–667): sanitize / extract / product-repo roots + user-guide sentence + three Rust intercepts (`memory.py`, `validate-plan.py`, `session_reader.py`).

### Linked docs already used

`docs/upstream-history.md`, import/onto logs, `docs/git-workflow.md`, many `doc/dev/research/*`, user-guide chapters 03/04/05/06/08/16/17/19/20/22, `doc/dev/upstream-regression-filters.md`, `RESIDUAL.md` §4 / Validate honesty, campaign `operator-orchestration-2026-07.md`, external DOGE spec `0001_DOGE.md`.

---

## 3. What is thin or stale

**Checkbox-only / report-only (no named `fn` in FORK):** UDAX T0–T6, ULID, usage.jsonl append path (76) vs `/spend` ingest (class 3), OpenRouter, keyring time-box, economic mode, Token Economy pillars, auto-compact 95% live-apply, auto-run `/implement`, shared rate limits, Updates, `/rebuild` SHA-aware accept (only neighbor fail-install is in the cheat sheet), plan.json honesty, auto-seed `ask:`, default-agent todo board, same-batch `exit_plan_mode` (named in dogfood wave, not land class 4), rustc 1.97.1, AUR/Nix/justfile, always-three-layer process law, todo fib / archive / notes / git-recon, Prefer Rust A1–A4, most DOGE polish (Wave 2, activity glyphs, Clear finished, click tasks, Worked-for, pause/resume/stop, continue-interrupted, killall e2e, OAuth 403, multi-track `meta.taskId`), btw B1/B2, ASCII scrub (settings row is in class 2; stream scrub is not a land class).

**Half-labels and outdated meter language (scrub when rewriting):**

- Line 81: “out of allowance”, “credit/allowance exhausted-fingerprint”, “extras-paid SuperGrok”, “free-period-safe path.”
- Line 82: “live free-period poll”, `activeDriver` value `supergrok_free_period`, `allow_spend_when_free_period_debit_unproven`, “C4 (server free-period debit).” Wire names may stay after a plain thought.
- Line 83: personal SuperGrok **“included weekly”** and SuperGrok **“dollar extras”** / “SuperGrok $ extras.” Use **included SuperGrok period limits** and **SuperGrok dollar credits**. SuperGrok is paid. Never “free SuperGrok.”
- Compact chrome still says `SuperGrok extras · $N` (82). That is UI copy; FORK should say it is the SuperGrok dollar credits meter.
- Class 3 title **“grok-oss SQL extras”** (563) means extra SQL, not SuperGrok dollar credits. Rename in prose so a tired reader does not mash meters.

**Structural thinness:** Chrome and plan CTAs sit under ### Process (174–449), so a writer expanding only ### Product will miss them. Dogfood (451–507) is dated **2026-08-09** and lists “in flight / not shipped” that later `[x]` bullets already claim (plan P1/P2 are `[x]` on 279–318). Dual-auth Product bullet (81) is one giant paragraph; Business/Team-before-personal and one-process flock live mainly in the cheat sheet (643–651), not in that bullet.

**“Shipped” honesty:** FORK uses `[x]` everywhere in the inventory. Treat as **FORK claims X; verify in code later.**

---

## 4. Land-checklist classes (copy)

Current text (547–567), enough to strengthen without re-reading the file:

> Do not claim "Surmount seams survived" until this list is done. `just check` is quality only. It cannot fail a deleted catalog test. A chrome-only inventory is a failed land, not a complete report.
>
> 1. Run `just upstream-assert-process-pins` (or `./scripts/assert-process-pins.sh HEAD`). Files and light sniffs only.
> 2. Run the named cargo filters for the **seven inventory classes** below. Use the existing test names in the catalog. Do not invent a filter that is not in the tree.
> 3. `rg` each required identifier for a matching `fn`. A named filter with no matching `fn` is a failed land.
> 4. **Helper-green is a failed land.** Forbidden as proof: a `--version` test that only checks stdout contains the substring `grok` (that is how `grok 1.0.3` stayed green); catalog-exists without paint; schema-exists without `/spend` ingest; serde `hide_header` without a `/settings` row and a runtime reader; rank helpers without `sampling_config` hop keys.
> 5. Dogfood screenshots (rails, five CTAs, compact included SuperGrok period limits meter, SIGUSR1 after a failed install) stay an operator check. They are not the only check.
>
> **Seven inventory classes** (each must be proven by a named cargo test or a named filter-catalog entry):
>
> 1. **CLI identity.** The product command is **grok-oss**. `grok-oss --version` first token is `grok-oss`, not bare `grok`. Resume and relaunch hints are `grok-oss --resume`.
> 2. **Config is a surface, not a field.** A toml field that deserializes is not shipped if `/settings` has no row and no runtime reader. Restack lost unread keys (`hide_header`, always-expand thinking, plan park, worktrees, ASCII scrub at launch, bubble copy) and leftover `/settings` rows plus DOGE in the theme picker.
> 3. **grok-oss SQL extras.** `$GROK_HOME/grok_oss.db` is the Token Economy ledger, not the session store. Schema v1 surviving is not enough. `/spend` must ingest `usage.jsonl` and write `reconciliation_run` (not `DoubleEntryReport::default()`).
> 4. **DOGE / Surmount chrome.** A theme file existing is not paint. Land must keep paint/render tests for human green rails plus box caret, magenta model / running agent, the compact **included SuperGrok period limits** meter, the titled composer frame (`prompt_border_active` white, yellow title only), and the five-CTA plan panel.
> 5. **`FORK_PATHS` restore is docs and scripts only.** Product seams inside `xai-grok-*` survive onto only via cherry-pick plus cargo tests. `scripts/assert-process-pins.sh` proves files exist. It does not prove contracts.
> 6. **Inventory must not be chrome-only.** After restack the required classes are at least: chrome/paint, `/settings` plus unread config, grok-oss ledger `/spend`, CLI branding, dual-auth hop after included SuperGrok period limits are full, last-session on start.
> 7. **Product skills are not a Python runtime.** A restack that installs non-excepted Python under product skills, or that drops the Rust intercept for `memory.py` / `validate-plan.py` / `session_reader.py`, is a failed land. Office/docx/pptx/xlsx/pdf scripts and those three allowlisted CLI stubs are the only exceptions. User-guide `08-skills.md` must keep that sentence. Named cargo tests must fail if the filter or the intercept is gone.

**Class 6 vs the numbered 1–7:** item 6 is a meta-rule (do not stop at paint). The seven *named* product classes in the cheat sheet are: CLI identity, config surface, `/spend`, chrome, dual-auth hop, last-session, skills-not-Python. Class 5 is process (assert ≠ contracts). Class 6’s “at least” list omits skills-not-Python even though class 7 and the cheat sheet include it. Writer should make those lists match.

**Extras already treated as neighbors, not a numbered class:** titles-on / no `hide_title_bar`, stuck-retry, `shell_collision`, `/rebuild` fail-install, `exit_plan_mode_soft`.

---

## 5. Missing recent shipped work (heading vs defense)

Operator-flagged items vs what FORK actually defends:

| Seam | FORK heading / claim | Named test + path in FORK? |
|------|----------------------|----------------------------|
| DOGE / Surmount chrome (human green / agent magenta, titled composer, rails, compact included SuperGrok period meter, Clear finished, pause/resume chips) | Long Process bullets 174–365; class 4 names rails, caret, magenta, compact meter, titled frame, five-CTA | Class 4 + cheat sheet cover rails, caret, magenta rail, compact meter, titled frame. **Clear finished** (225–237): no land `fn`. **Pause/resume chips** (343–365): only in 2026-08-09 dogfood filters (`work_control_chrome_matrix`, `pause_button_click_dispatches_global_pause`), not the seven-class cheat sheet. Lower-left magenta throbber: catalog later notes missing paint `fn`s; FORK does not. |
| Always-on bubble copy click + wrap when first line is full | 253–257: glyph + settings row. Class 2 lists `bubble_copy_buttons` | Cheat sheet only `bubble_copy_buttons_on`. **No** `clicking_*` or first-line-full / wrap filters in FORK. Those names live in `doc/dev/upstream-regression-filters.md` class 2 / bubble section. FORK claims paint; click + wrap defense is missing here. |
| Plan five-CTA + present ≠ approve + modal-free typing | 97, 264–318; class 4 names five-CTA | Cheat sheet: `plan_approval_footer_paints_five_cta_vocabulary`. Present ≠ approve, empty Enter, sticky decision, revise in-flight: named only in dogfood wave (494–498), not land class 4. **FORK claims** the contracts; **verify** later. |
| Last-session | 77 + class 6 | One named test. Thin but present. |
| Branding `grok-oss` | 78 + class 1 | Strongest named-test block in Product. |
| Dual-auth + Business/Team included before personal + sibling included before SuperGrok dollar credits + one-process limits flock | 81 is credit/429/hop prose; 82–83 meters | Cheat sheet class 5 **does** name sibling-before-credits, Business-before-personal, `limits_snapshot_*` flock. Product bullets do **not** say those three in hierarchical one-liners. Flock is “FORK cheat sheet claims; verify in code later.” |
| `/spend` usage.jsonl ledger | 76 is append-only log; 87 pillar (3); class 3 | Class 3 names ingest + `reconciliation_run`. Append-path (`usage_log`) is only in the catalog residual block, not FORK land. |
| Unread config + `/settings` rows | Class 2 + settings_e2e | Present. Recap / cancel-subagents rows (258–261) are **not** in the e2e filter list. |
| Skills sanitize + Rust intercepts | 157–173 Prefer Rust; class 7 | Class 7 is strong. Skills subsection (510–526) has no tests. |
| Always-three-layer agent depth | 114 process `[x]` | Process law + `AGENTS.md`. **No** product cargo test. Do not invent one unless a prompt/pin contract exists. |
| Bubble-copy catalog contracts | See wrap row | **Missing from FORK cheat sheet.** Pull from the catalog when expanding. |
| `from_config` empty-cache miss | Cheat sheet has `resolve_from_config_no_config` (no config → DOGE) | That is **not** an empty theme-cache miss. **Nucleo / empty-cache miss is absent from FORK.** |
| `/rebuild` SHA-aware | 91 claims SHA-aware identity + SIGUSR1 peers | Neighbor: `failed_install_must_not_replace_or_signal_peers` only. **No** named test for “same semver + newer SHA accepts.” |
| rustc 1.97.1 wins | 104 + impl report | Checkbox + flake/toolchain paths. **No** land filter. Easy to lose on import if `rust-toolchain.toml` is not in `FORK_PATHS` (verify paths later). |
| Nucleo reuse-per-root | **Not in FORK at all** | Shipped in residual / onto log / `docs/upstream-history.md`. Writer must add a hierarchical bullet + named tests. |

Dogfood “not shipped” (470–481) still lists auto-resume-after-error and mid-sample freeze. Those are honesty items, not missing inventory of shipped chrome.

---

## 6. Style notes already in FORK

- Lines 69–70: one line here → code or linked doc. **Update this list when you ship.**
- Lines 547–558: named catalog filters; `rg` for `fn`; helper-green failed land; screenshots are operator-only.
- Lines 569–587: product seams inside `xai-grok-*` survive onto only via cherry-pick + named cargo tests. `just check` cannot fail a deleted catalog test.
- Line 598: “Deleting a red catalog test is not a restore.”
- Dual-pin: FORK + AGENTS + user-guide for product; host `~/.agents` for operator-only (523–526).
- Class 4 already uses complete meter language: **included SuperGrok period limits**.
- Catalog (`doc/dev/upstream-regression-filters.md`) is denser than FORK on bubble-copy click, combined remaining (5b), and paint Keep-table. Expand FORK toward that hierarchy; do not duplicate the whole catalog.

---

## Writer must add / strengthen

- Move or cross-link chrome / plan / pause / bubble-copy out of a 300-line Process dump into hierarchical Product (or a ### Chrome) bullets: **one line → named `fn` + crate**, not report-only.
- Make land class 6’s “at least” list include **skills-not-Python** (class 7) so the two lists match.
- Add missing land neighbors (or class 4/2 filters) that the catalog already has: bubble **click** + **first-line-full / wrap**, plan **present ≠ approve** / empty Enter / sticky decision, Clear finished non-overlap, pause/resume/stop chrome.
- New Product one-liners with tests: **nucleo reuse-per-root**; **theme `from_config` empty-cache miss** (do not reuse `resolve_from_config_no_config` as if it were that contract); **`/rebuild` same-semver newer-SHA accept**; **rustc 1.97.1 wins** after an older upstream toolchain.
- Dual-auth Product bullet: split into included SuperGrok period limits → SuperGrok dollar credits → console; **Business/Team included before personal**; **sibling included before SuperGrok dollar credits**; **one-process limits flock** (`limits_snapshot_*`). Scrub “allowance”, “included weekly”, “free-period”, “extras” as if they were the meter names.
- usage.jsonl: keep append-log (76) distinct from `/spend` ingest (class 3); name both tests.
- Refresh or date-box the 2026-08-09 dogfood section so it cannot demote later `[x]` claims.
- Always-three-layer: keep as process pin; if a prompt contract exists, add a named test, else say “docs + `assert-process-pins` only.”
- Every new `[x]` must include a catalog `fn` or an explicit “verify in code later.” Never ship another checkbox-only seam.

End of map. The writer should edit FORK.md and the catalog together; this file is not the inventory.
