# CI cluster: 239 unit failures (`just check` / `just ci`)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Source:** nextest summary in session prompt
`/home/hunter/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fsurmount%2Fgrok-build/019faf9d-ef93-7d93-b34b-9f19b6345613/prompts/prompt_436.txt`
**Agent:** L2 explore (read-only). No shell in this worker: panics below are from the CI paste plus code-contract inference, not a fresh local re-run.

---

## Executive summary

| Item | Value |
|------|--------|
| **Run** | 29311 tests; 29072 pass (1 slow, 1 flaky); **239 failed**; 426 skipped; ~518s wall |
| **Exit** | 100 (`cargo-ci` / `test-unit` / `ci`) |
| **Verdict** | **Primarily (B)** many independent onto-merge / hybrid runtime breaks, **plus 2–3 (A) systemic clusters** that explain large slices, **plus minor (C)** host flakiness |
| **Not** | One single env var or nextest isolation bug for all 239 |

Instant fails dominate (~0.05–0.3s). That pattern means assert/setup failures, not load/timeouts. Timeouts/aborts are a small minority.

---

## Package counts (239 = final `TRY 2 FAIL|ABRT` list)

| Package / binary | Count | Typical duration | Notes |
|------------------|------:|------------------|--------|
| **xai-grok-pager** (lib) | **148** | 0.05–0.33s | Mass dispatch / acp_handler / key_owner / session / status / turn |
| **xai-grok-shell** lib + integration | **59** | mostly instant; 4 ABRT ~6–9s; 2 auth timeouts | Includes `::team_managed_config` (30), external_auth (2), session acp |
| **xai-fast-worktree** | **12** | ~0.09–0.18s | All `auto_gc` + `git::worktree` registration tests |
| **xai-grok-workspace** | **10** | ~0.09–0.15s | Entire `export_github` suite |
| **xai-grok-pager-render** | **2** | ~0.03s | Auto dark → DOGE theme |
| **xai-grok-tools** | **2** | ~0.05–0.06s | `is_read_only` + contract snapshot |
| **xai-grok-agent** | **1** | 0.08s | Encrypted prompt templates stale |
| **xai-grok-config** | **1** | **0.007s** | `claim_paths_are_inert_in_dark_build` |
| **xai-grok-hooks** | **1** | 0.03s | `/dev/tty` detach |
| **xai-grok-pager-minimal** | **1** | 0.15s | Dim thinking rail |
| **xai-grok-sampler** | **1** | 0.04s | `status_user_message_matrix` (cf edge) |
| **xai-grok-update** | **1** | 0.26s | install-internal smoke fallback |
| **Total** | **239** | | |

**Flaky (not in 239):** `xai-grok-shell terminal::pty_session::tests::close_pty_kills_a_background_grandchild` — TRY 1 FAIL 300s (`grandchild … survived the pty close`), TRY 2 PASS 0.11s.

**Timing split (approx):**

| Class | Share | Meaning |
|-------|-------|---------|
| Instant (≤0.4s) | ~230 | Shared setup assert, wrong product state, snapshot mismatch |
| Mid (1–10s) | ~5 | ABRT / hang cutoffs on acp session tests |
| Timeout (60s+) | 2 | External auth fell through to real browser OAuth |

---

## Classification (ranked)

### 1. (B) Many independent onto-merge / hybrid runtime breaks — **primary**

- Recent mop reports: pager **compile** green, runtime tests **not** re-run (`impl-upstream-pager-tests-compile-2026-08-11.md` residual: “Compile green ≠ runtime green”).
- 148 pager fails span many modules (dispatch, session lifecycle/fork/load, status privacy, key_owner, acp_handler, scrollback layout). That is not one fixture panic string for the whole crate; it is half-merge behavior drift after onto.
- Shell session / prompt-queue / plan-gate / recap tests look like the same class (onto hybrid).

### 2. (A) Systemic cluster — **armed signed-policy / “dark build” mismatch**

Product build embeds prod `v1` pubkey; `verification_active()` is **true** by default (`crates/codegen/xai-grok-config/src/signed_policy.rs`).

| Symptom | Why |
|---------|-----|
| `claim_paths_are_inert_in_dark_build` (0.007s) | Asserts `!verification_active()` **without** `test_seam::with_dark`. Armed build fails the first line. |
| `xai-grok-shell::team_managed_config` (**30** tests, all ~0.1s) | Suite comments assume “verification is inactive here”; mocks write unsigned policy. Armed gate changes fail-closed / compromise / eviction behavior. |

Likely one implementer scope: either wrap suite in dark seam for tests that need keyless, or update mocks to signed envelopes when verification is armed.

### 3. (A) Systemic product bug — **external auth interactive path sets `GROK_AUTH_EXPIRED=1`**

Contract (`external_auth_conforming_provider.rs`):

1. Headless refresh: provider gets `GROK_AUTH_EXPIRED=1`, declines → OK.
2. Interactive sign-in: provider must run with **`expired=unset`**, mint SSO token.
3. Must **not** fall through to browser OAuth (hangs for 60s).

CI evidence (stderr):

```text
SSO session lapsed; cannot mint without the user
Signing in with browser instead...
Open this URL to sign in:
  https://auth.x.ai/oauth2/authorize?...
panicked: the sign-in must reach the provider's interactive branch, not the browser login: Elapsed(())
```

Product path (`auth/flow.rs`):

```rust
// run_auth_flow_inner after failed refresh:
if let Some(ref cmd) = grok_com_config.auth_provider_command {
    let is_refresh = reauth || auth_manager.is_expired();  // still true: expired disk cred
    match run_external_auth_provider(cmd, auth_manager, is_refresh, on_stderr).await {
        Ok(result) => return Ok(result),
        Err(e) => {
            eprintln!("Signing in with browser instead...");
            // falls through to real OIDC browser
        }
    }
}
```

And `run_external_auth_provider` only sets `GROK_AUTH_EXPIRED=1` when `is_refresh`.

Contrast: `mint_session_noninteractive` correctly calls `run_external_auth_provider(..., false, ...)`.

**Root cause (high confidence):** interactive / force-login still treats an expired on-disk external credential as a **refresh**, re-sends `GROK_AUTH_EXPIRED=1`, provider declines again, browser fallback. Not host keyring pollution; test deliberately seeds expired External auth + dead endpoints.

Related: `external_auth_expired_credential` (1.3s) same family.

### 4. (C) Environmental / host — **minor**

| Case | Signal |
|------|--------|
| `hook_child_cannot_open_dev_tty` | Expects setsid detach; fails if session leader / controlling tty behavior differs under nextest |
| `close_pty_kills_a_background_grandchild` | Flaky 300s then pass; process-group kill race |
| fast-worktree / export_github | Real `git` in temp dirs; if one plant helper breaks, whole cluster fails instantly |

Not the main explanation for pager/shell mass.

### 5. Small independent oneshots (also (B))

| Test | Likely contract |
|------|-----------------|
| `test_encrypted_templates_not_stale` | Run / regenerate `scripts/encrypt_templates.py` after template edit |
| tools `capabilities_is_read_only_matches_metadata` | Metadata vs hub `capabilities().is_read_only` drift |
| tools `non_pi_finalized_contract_snapshot_is_unchanged` | Checked-in JSON schema snapshot drift |
| pager-render auto dark → DOGE | Mock appearance or default dark map vs DOGE pin |
| pager-minimal dim rail | Paint/API after dim_accent removal |
| sampler / update | Snapshot / install-internal behavior |

---

## Sample panic / fail messages (5–8 packages)

Live `cargo test` re-run was **not** executed in this explore worker (no shell tool). Messages:

1. **xai-grok-shell::external_auth_conforming_provider** (60s timeout)
   `the sign-in must reach the provider's interactive branch, not the browser login: Elapsed(())`
   stderr: `Signing in with browser instead...` + real `auth.x.ai` URL.

2. **xai-grok-shell** pty (flaky, not in 239)
   `grandchild <pid> survived the pty close` @ `pty_session.rs:976`.

3. **xai-grok-config** `claim_paths_are_inert_in_dark_build` (0.007s)
   First assert: `assert!(!crate::signed_policy::verification_active())` — armed build has embedded `v1` key.

4. **xai-grok-hooks** `test_hook_child_cannot_open_dev_tty`
   Expected: `HookRunnerResult::Success` after child cannot open `/dev/tty`; failure message names that contract.

5. **xai-grok-agent** `test_encrypted_templates_not_stale`
   Expected message: `"prompt.md encrypted bytes are stale — run scripts/encrypt_templates.py"` (or sibling templates).

6. **xai-grok-tools** `capabilities_is_read_only_matches_metadata`
   Expected: list of `{name}: ToolMetadata::is_read_only()=… Tool::capabilities().is_read_only=…`.

7. **team_managed_config / pager mass**
   No stderr in the truncated nextest summary (status-only lines). Infer: assert after mock sync / app dispatch; not network timeouts.

**Parent should** spawn a process-mop or implementer with shell to capture one panic line each if needed before fix PRs.

---

## External auth path (detail for implementers)

| Step | Expected | Actual (CI) |
|------|----------|-------------|
| Seed expired External + `auth_provider_command` | — | OK |
| `try_ensure_fresh_auth` | None; log `expired=1` | OK (provider decline text seen) |
| `ensure_authenticated(..., false, None)` | Provider **interactive** (`expired=unset`), mint `SSO_TOKEN` | Second provider call still headless-style → decline → **browser** OAuth to live `auth.x.ai` → 60s timeout |

**Fix direction (not implemented here):** when entering interactive sign-in after a declined headless refresh (or when `force_interactive` / “user must complete SSO”), call external provider with `is_refresh = false` so `GROK_AUTH_EXPIRED` is unset. Do not fall through to browser while `auth_provider_command` is configured and the failure was “declined headless,” not “provider missing.”

Mirror test: `mint_session_noninteractive` already uses `false`.

---

## Recommended fix order (disjoint implementer scopes)

Parallel-safe scopes; do not race same files.

| Order | Scope | Packages / paths | Est. impact |
|------:|-------|------------------|-------------|
| **1** | **External auth interactive refresh flag** | `xai-grok-shell` `auth/flow.rs` (+ maybe `external_auth.rs`); tests `external_auth_*` | 2 hard timeouts + product correctness |
| **2** | **Signed-policy dark vs armed tests** | `xai-grok-config` claim_tests; `team_managed_config.rs` seam or signed mocks | **~31** instant fails |
| **3** | **Pager runtime half-merge** (split sub-scopes) | `xai-grok-pager` by area: (3a) dispatch/session lifecycle, (3b) key_owner/plan/status, (3c) acp_handler/queue, (3d) scrollback layout | **~148** |
| **4** | **Shell session / plan / queue / recap** | `xai-grok-shell` `session::acp_session::*`, `acp_session_setup_wire`, prompt_queue | ~20 + ABRTs |
| **5** | **Git worktree / export** | `xai-fast-worktree` (12) + `xai-grok-workspace` export_github (10) | 22; one plant/git helper each |
| **6** | **Oneshoots** | agent encrypt templates; tools registry snapshots; hooks tty; pager-render theme auto; pager-minimal rail; sampler; update | ~9 |
| **7** | **Flaky pty grandchild kill** | `pty_session` / local_terminal kill path | reliability only |

Do **not** treat “set GROK_HOME differently in CI” as the global fix. Nextest process-per-test is already the isolation model; external_auth binaries even document single-test-per-binary for `GROK_HOME` memoization.

---

## What this is **not**

- Not “CI is completely broken / wrong binary”: 29072 pass.
- Not pure host keyring pollution (external auth seeds temp `GROK_HOME` + dead endpoints).
- Not one dark-build feature flag for all crates (only config/team_managed share verification).
- Not fixed by compile-only mop; runtime asserts remain.

---

## Suggested parent next steps

1. Board: `bug:ci-239-test-cluster` (this report).
2. Spawn **(1)** external-auth implementer and **(2)** signed-policy/team_managed implementer in parallel.
3. After those green, fan out **pager** by module (3a–3d) + shell session (4).
4. Optional: one shell sample pass per package to pin exact panic lines into this report or a follow-up.

---

## 10-line summary

1. 239 fails; pager 148 + shell 59 + fast-worktree 12 + workspace 10 + small oneshots.
2. Mostly instant asserts → many product/test contract breaks, not timeouts.
3. **Primary:** onto half-merge runtime drift (pager + shell session).
4. **Systemic A1:** armed signed-policy vs tests that assume dark → claim + team_managed (~31).
5. **Systemic A2:** interactive external auth reuses `is_refresh` from expired disk → browser hang (2).
6. **Env C:** hooks tty, flaky pty kill, git helpers (small).
7. Fix order: auth flag → dark/armed tests → pager slices → shell session → git → oneshots.
8. No full product fix in this report.
