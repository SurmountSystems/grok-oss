# Upstream regression filters land — onto tip quality gate

**Date:** 2026-08-10
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Prior:** `.agents/reports/impl-upstream-post-1.0-integrate-resume-2026-08-10.md`
**Catalog:** `doc/dev/upstream-regression-filters.md` + FORK cheat sheet

---

## Executive status

| Item | State |
|------|--------|
| **Tree at start** | Clean on onto tip (docs tip `2c34b6c8`); stashes kept |
| **Assert** | `./scripts/assert-process-pins.sh HEAD` **PASS** |
| **Package filters green** | rate-limit (solo), shared hide_header, sampling-types credits/auth, pager-render DOGE, sampler retry/headers/dual-auth unit + headers integration, tools densify/TOON |
| **Blocked** | `xai-grok-shell` / `xai-grok-pager` **do not compile** (large merge fallout) |
| **`just check`** | **Skipped** (shell/pager compile blocks full gate) |
| **Push/PR** | **Not done** |
| **Stashes** | `recon-temp-work-b-wip-2026-08-10` and `recon-resume-local-dirt-2026-08-10` **kept** (not dropped) |

**Bottom line:** Process pins green. Core non-shell/pager product filters that compile are green after surgical mop. Full catalog (pager titles, shell dual-auth, plan soft-park, settings_e2e) is **blocked** until shell/pager merge fallout is mopped. Working tree has uncommitted mop edits; no agent push/commit.

---

## Tree / stashes

```
## onto-xai/b13fa526f511
stash@{0}: recon-resume-local-dirt-2026-08-10
stash@{1}: recon-temp-work-b-wip-2026-08-10
```

Uncommitted mop only (this gate). Stashes not touched.

---

## Filter log (command → result)

### Process

| Command | Result |
|---------|--------|
| `./scripts/assert-process-pins.sh HEAD` | **PASS** (24 files + 5 dirs) |

### Package-scoped catalog filters

| Command | Result | Notes |
|---------|--------|-------|
| `cargo test -p grok-rate-limit --lib` | **PASS** (solo); **flaky FAIL** once in multi-package batch | `observe_status_writes_on_429` asserted `remaining=0ns` under parallel load; 3× solo retest green; full 15/15 solo green |
| `cargo test -p xai-grok-shared --lib -- hide_header stale_hide_title` | **PASS** | |
| `cargo test -p xai-grok-sampling-types --lib -- credit_exhausted credentials_rejected forbidden_bad_credentials` | **PASS** | |
| `cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config doge_accent_user_is_pure_green doge_accent_system_is_pure_cyan doge_roles` | **PASS** (5 tests) after DOGE mop | |
| `cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason retry_footer_backoff stream_headers_timeout_defaults rotate_ exhausted memo fingerprint hop_reason live_rebind` | **PASS** after sampler mop | |
| `cargo test -p xai-grok-sampler --test stream_headers_timeout` | **PASS** | |
| `cargo test -p xai-grok-tools --lib -- densify_mcp densify_structured toon` | **PASS** after tools mop | |
| `cargo test -p xai-grok-pager --lib -- hide_header window_title titles_on_session default_title_items shell_collision retry_chrome… user_prompt… bubble_copy_ clear_completed_todos product_cli_name…` | **FAIL compile** | Depends on shell/tools product paths; shell does not compile |
| `cargo test -p xai-grok-shell --lib -- …` (any dual-auth / stream_resumed) | **FAIL compile** | See residual |
| `cargo test -p xai-grok-pager --test settings_e2e -- hide_header` | **Not run** | Pager/shell blocked |
| Plan soft-park / dual-principals / interject / btw blocks | **Not run** | Shell/pager blocked |
| `just check` | **Skipped** | Full gate not meaningful until shell+pager compile |

---

## Mop applied this gate (surgical compile fallout)

Uncommitted on `onto-xai/b13fa526f511` (26 paths). Intent: restore product seams that onto merge left half-applied so catalog packages load.

1. **`prod/mc/cli-chat-proxy-types`:** remove duplicate `MANAGED_*` / `is_server_nonce_shape` definitions.
2. **`xai-grok-test-support`:** dedupe `log_timing`; `auth_rejection` → `pub(crate)`.
3. **`xai-grok-tools`:** restore `WarmShell` variant; 3-arg `ShellState::init`; `static_shell` `String` binary path; densify MCP re-exports + pre-cap densify; product `format_default_prompt` noop reminder; confusable hint native-edit path (no python/sed steer).
4. **`xai-grok-sampling-types`:** restore `SentCredential` + `Auth { message, credential }` + `auth_unknown` (product dual-auth wire provenance).
5. **`xai-grok-sampler`:** deps `xai-grok-auth` + `xai-grok-extra-ca`; `SENT_BEARER_PREFIX_LEN`; Auth constructors/patterns updated; `SamplingErrorInfo` fields complete.
6. **`xai-fast-worktree`:** merge duplicate `db` re-exports.
7. **`xai-grok-pager-render`:** restore `ThemeKind::Doge` + `mod doge` + constructors; `accent_feedback` field across themes; remove duplicate kitty_flags helpers in `terminal/mod.rs`.

**Not committed** (recon-unsigned available if operator wants tip commit). No push.

---

## Residual (hand)

### High: shell / pager do not compile

`cargo test -p xai-grok-shell --lib -- --list` fails with **many** errors of two classes:

1. **Duplicate definitions** (merge double-keep): `slash_command_tags_*`, `resolve_worktree_auto_gc*`, `build_session_doc`, `collect_all_indexable_content_single_pass`, …
2. **Missing product symbols** (tip-only side won; product call sites remain): `SilentRefresh`, `QueueInputRequest`, todo helpers (`plan_entry_from_todo`, `clear_completed_todos`, …), persistence title sanitizers, managed MCP types, `PAGER_COMMAND_KEYS`, `AcuSkillSource`, cancel/shutdown types, …

Until shell compiles, catalog filters that need pager/shell (titles, shell_collision, dual SuperGrok `/limits`, plan soft-park, settings_e2e, human rail under pager) cannot run.

**Suggested land mop:** dedicated shell-first pass: dedupe lib/config modules, then restore missing product modules from `origin/main` / onto product picks (todo, persistence, managed_mcp, auth silent refresh). Then pager.

### Medium: rate-limit timing flake

`observe_status_writes_on_429` can see `remaining=0ns` when the full suite races. Solo green. Consider stronger wait or isolated temp store path if it fails CI.

### Skipped gates

- `just check` / full nextest
- Dual-auth shell filters (`resolve_credentials`, `dual_supergrok`, multi-principal limits)
- Plan soft-park pager filters
- settings_e2e hide_header

### Stashes (do not drop)

| Stash | Role |
|-------|------|
| `recon-temp-work-b-wip-2026-08-10` | Work B WIP |
| `recon-resume-local-dirt-2026-08-10` | Local dirt parked at resume |

---

## Hand push/PR (not run)

After shell/pager mop + green catalog + optional `just check`:

```bash
# optional: commit mop on onto (recon unsigned if no GPG TTY)
# ALLOW_UNSIGNED_COMMIT=1 … only on onto tool branch

git push -u origin onto-xai/b13fa526f511
# then PR to main when gate is honest
```

---

## Success criteria

| Criterion | Met? |
|-----------|------|
| Clean tree or intentional dirt only | **Yes** (mop dirt intentional) |
| Assert HEAD green | **Yes** |
| Catalog filters as practical | **Partial** — green where packages compile |
| Fix clear onto mop compile fallout | **Yes** for proxy-types, test-support, tools, sampling-types, sampler, fast-worktree, pager-render DOGE |
| Shell/pager filters green | **No** (compile blocked) |
| `just check` | **Skipped** |
| No push/PR | **Yes** |
| Report path | **This file** |

---

## 10-line summary

1. Onto tip clean at start; stashes preserved.
2. Assert process pins **green**.
3. Mopped clear merge compile fallout (nonce dups, tools WarmShell/densify/noop, SentCredential/Auth, sampler deps, DOGE ThemeKind, worktree re-exports).
4. Green: rate-limit (solo), shared hide_header, sampling-types credits/auth, pager-render DOGE, sampler retry/headers/dual unit + integration, tools densify/TOON.
5. Red/blocked: shell and pager do not compile (duplicate + missing product symbols).
6. Pager catalog filters and dual-auth shell filters not runnable until shell mop.
7. `just check` skipped.
8. Rate-limit one flake under parallel batch; solo 15/15.
9. Mop left **uncommitted** on onto branch; no push.
10. Next land residual: shell compile mop → pager filters → `just check` → hand push/PR.
