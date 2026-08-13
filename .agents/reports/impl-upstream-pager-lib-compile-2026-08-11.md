# Pager lib compile mop — onto-xai land

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Prior:** `.agents/reports/impl-upstream-shell-pager-compile-2026-08-10.md`

---

## Executive status

| Item | State |
|------|--------|
| **`cargo check -p xai-grok-pager --lib`** | **GREEN** (warnings only) |
| **`cargo check -p xai-grok-shell --lib`** | **GREEN** (warnings only; one small public flush wrapper) |
| **Shell/pager unit tests / catalog filters** | Still blocked (not this mop’s scope) |
| **Stashes** | `recon-temp-work-b-wip-2026-08-10`, `recon-resume-local-dirt-2026-08-10` **kept** |
| **Push** | **Not done** |

**Bottom line:** Pager **library** compiles on the onto tip after surgical half-merge fixes. Shell lib stays green. Product UI seams (plan approval prompt chrome, privacy hits, `/delete` confirm, title reset TaskResults) restored where half-merge dropped them.

---

## Strategy

| Layer | Choice |
|-------|--------|
| Shell | Keep green tip APIs; add only `MvpAgent::flush_all_sessions` so pager shutdown does not need private `activity` |
| Pager call sites | Adapt to tip types (`DeferredModelSwitch`, `PrivacyBannerRects` opt_in/opt_out/terms/policy, `SettingsFetch::into_option`, `PagerLocalSnapshot.scheduler_background_loops`) |
| Missing product variants | Restore from monorepo tip / Surmount product (`DeleteCurrentSession*`, `CancelTrigger::DashboardStop`, `TaskResult::ResetSessionTitle*`) without full `AfterSessionDelete` Effect cascade |
| Restores | No whole-module `git show fixes-2:` blob restore this pass; surgical field/method/variant adds |

---

## Fixes this pass (classes)

### Shell (minimal)

1. **`MvpAgent::flush_all_sessions`** — public async wrapper over private `activity` (`agent_ops.rs`). Pager ACP spawn uses `agent_rc.flush_all_sessions(...)`.

### Pager UI / half-merge

2. **`PromptStyle`** — `placeholder_when_focused` on main chat prompt style (`agent_view/render.rs`).
3. **Privacy banner hits** — map `opt_in` / `opt_out` / terms|policy → existing `hit_accept` / `hit_customize` / `hit_legal`.
4. **`render_permission_view`** — pass `permission_pattern_edit` (8-arg tip signature).
5. **`CancelTrigger::DashboardStop`** + wire `"dashboard_stop"`.
6. **`Action::DeleteCurrentSession` / `DeleteCurrentSessionAnswered`** + open/answer handlers + router arms; local question maps to **Answered** with `idx == 0`.
7. **`TaskResult::ResetSessionTitleComplete` / `Failed`** + task_result handlers (pin restore / committed fan-out).
8. **`DeferredModelSwitch` end-to-end** — `deferred_model_switch_from_cli`, `take_deferred_model_switch`, mismatch-answered stash.
9. **`PagerLocalSnapshot.scheduler_background_loops`** on five partial inits (dashboard slash, prompt slash, settings open/refresh/build).
10. **`SessionPickerEntry.last_turn_summary`** in helpers parse path.
11. **Dashboard focus row** — `*id == *closed` PartialEq fix.
12. **`fetch_settings_blocking` → `.into_option()`** on RefreshGate (SettingsFetch vs Option).

### Earlier mop (same branch, prior agent turn)

Product modules/setters, SlashCommand trait, AgentView methods, rewind/Btw/permission exhaustiveness, dups, restore opts, kitty flags, etc. See prior report + working tree.

---

## Restores documented

| Path | Source | Why |
|------|--------|-----|
| Delete current session actions + handlers | Monorepo tip `b13fa526` product shape (adapted: no `AfterSessionDelete` on current `Effect::DeleteSession`) | Half-merge kept `/delete` slash + question kind but dropped Action/router |
| CancelTrigger DashboardStop | Monorepo tip | Product arm_dashboard_stop |
| TaskResult ResetSessionTitle* | Monorepo tip + existing effects arm | Effect arm landed without TaskResult variants |
| DeferredModelSwitch | Product struct already present; CLI/take helpers still on tuple | Type unify |
| MvpAgent flush wrapper | New thin public API | Tip privacy of `activity` |

**Not restored this pass:** full `AfterSessionDelete` on `Effect::DeleteSession` / complete TaskResult (would cascade router + effects + tests). `/delete` still issues `Effect::DeleteSession { source: "current", ... }` without after-nav; residual for later mop / tests green.

---

## Residual

- Pager **tests** / `--all-targets` still red (half-merge tests, AfterSessionDelete, seed fields, etc.).
- Shell **lib tests** still red (~297 from prior report).
- Catalog shell/pager filters still blocked until tests compile.
- `/delete` post-success navigation (welcome vs dashboard) incomplete without `AfterSessionDelete`.
- Privacy hit semantics: opt_out mapped to `hit_customize` (legacy hit names); legal hit is terms-preferred.

---

## Verify commands

```bash
cargo check -p xai-grok-pager --lib   # GREEN
cargo check -p xai-grok-shell --lib   # GREEN
git stash list                        # recon-temp-work-b-wip + recon-resume-local-dirt present
```

No push. Index left **staged** for recon-unsigned mop commit; agent commit failed
(GPG needs `/dev/tty` / passphrase — `ALLOW_UNSIGNED_COMMIT=1` alone does not
skip `commit.gpgsign=true` signing). Operator handoff (onto recon exception):

```bash
# after review of staged index:
ALLOW_UNSIGNED_COMMIT=1 git commit --no-gpg-sign -m "onto mop: xai-grok-pager lib green on tip" \
  -m "Half-merge UI/API alignment; shell lib stays green. Report: .agents/reports/impl-upstream-pager-lib-compile-2026-08-11.md" \
  -m "Recon intermediate: ALLOW_UNSIGNED_COMMIT under onto-xai recon exception."
```

Or signed TTY: `git commit -S` with the same message body.
