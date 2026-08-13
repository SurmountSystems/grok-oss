# Shell tests compile mop — onto-xai land

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Prior:** `.agents/reports/impl-upstream-shell-pager-compile-2026-08-10.md`
**Catalog:** `doc/dev/upstream-regression-filters.md`

---

## Executive status

| Item | State |
|------|--------|
| **`cargo check -p xai-grok-shell --lib`** | **GREEN** (warnings only) |
| **`cargo check -p xai-grok-shell --tests`** | **GREEN** (0 errors; warnings only) |
| **Prior test compile** | ~297 → ~58 → **0** |
| **Stashes** | `recon-temp-work-b-wip-2026-08-10`, `recon-resume-local-dirt-2026-08-10` **kept** |
| **Push** | **Not done** |
| **Pager lib** | Still dirty / not this mop’s gate |

**Bottom line:** Shell library and full shell **test** targets compile on the onto tip. Strategy stayed main-shaped HashMap sessions + tip monorepo APIs; restored missing dual-auth / plan-mode helpers rather than inventing a second product.

---

## What was fixed (this pass + prior mid-flight)

### Dev wiring

- `pretty_assertions`, `ctor`, self-dep `xai-grok-shell` + `test-support` (Cargo.toml / prior pass)
- `session::testkit` under `cfg(any(test, feature = "test-support"))`

### Product helpers restored (tests + product path)

1. **`subagent_override_auth_rank_flags`** + **`parent_sampling_is_supergrok_session_only`** in `agent/subagent/mod.rs`; wired into `resolve_model_override_to_config` via `resolve_credentials_preferring_with_rank` (included SuperGrok period limits before console).
2. **`is_plan_mode_blocked_ask_user_tool_name`** + real **`filter_cursor_tools_by_plan_mode`** strip of questionnaire names in plan mode (`session_mode.rs`).
3. Tip has no `PROACTIVE_MIN_SLEEP`; **`upload/trace.rs`** test wait uses a fixed 1.5s window.

### Test / fixture API alignment

| Area | Fix |
|------|-----|
| HashMap sessions | Main-shaped agent tests; no tip registry |
| Cancel / queue | `CancelOptions`, `QueueInputRequest`, handle fields |
| Recap / btw | `handle_side_question(q, None, vec![])`; `answer.answer` |
| Persistence actor | `created_fresh`, `disk_full_tx` / `disk_full_rx`, weak summary tx |
| MCP merge | 5-arg `merge_managed_mcp_servers(..., &[], None, &compat)` |
| LSP spawn fixture | Drop tip-only fields (`process_scope`, `parent_non_interactive`, depth caps not on tip struct) |
| base64 | `use base64::Engine as _` in tool-layer image test |
| Sampling / usage / auth retry | TokenUsage / Todo / `SamplingErrorInfo` / `RefreshAuthAndResubmit` fields (prior) |

---

## Catalog sample (post-compile)

| Filter / test | Result |
|---------------|--------|
| `subagent_override_auth_rank_flags_fail_closed_when_config_missing_and_session_live` | **PASS** |
| `plan_mode_blocked_ask_user_name_matcher` / `plan_mode_tool_list_omits_ask_user_question` | **PASS** |
| `stream_started_emits_retry_state_stream_resumed` | **FAIL at runtime** (compile ok; assert: StreamStarted must persist `RetryState::StreamResumed`) |

`shell_collision` is a **pager** filter; not run here (pager still mid-dirt).

---

## Residual

1. **Runtime product:** stuck-retry / `stream_resumed` catalog test fails assertion (chrome clear contract). Separate product mop.
2. **Pager lib** still does not compile (prior ~290 class); out of shell-tests scope.
3. Shell test suite may have further **runtime** reds; this mop only guaranteed **compile**.
4. Large dirty pager/shared tree left unstaged unless included in a broader recon commit.

---

## Stashes / git

- Stashes **not** dropped.
- Shell mop + report **staged** (`git add` shell crate, Cargo.lock, this report).
- Agent recon-unsigned commit **blocked** in this environment: GPG needs `/dev/tty` for passphrase, and host permission deny blocks agent `--no-gpg-sign` even with `ALLOW_UNSIGNED_COMMIT=1`. Prior onto tips used `N` (unsigned) commits when the operator ran the escape on a real TTY.
- **Operator handoff (onto recon Yes row):**

```bash
cd /home/hunter/Projects/surmount/grok-build
# index already has shell mop paths staged; confirm:
git status -sb
ALLOW_UNSIGNED_COMMIT=1 git commit --no-gpg-sign -m "recon: shell tests compile green on onto tip" \
  -m "Make cargo check -p xai-grok-shell --tests green while keeping --lib green." \
  -m "Restore dual-auth subagent rank flags and plan-mode questionnaire strip; align fixtures." \
  -m "Report: .agents/reports/impl-upstream-shell-tests-compile-2026-08-11.md" \
  -m "Recon intermediate: ALLOW_UNSIGNED_COMMIT under 2026-08-10 recon exception."
# or: unlock GPG and git commit -S with the same message
```

- No push.
- Pager dirt left **unstaged** (separate mop).
