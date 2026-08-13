# Fix: armed signed-policy vs dark-build test cluster (~31)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 implementer
**Cluster source:** `.agents/reports/bug-ci-239-test-cluster-2026-08-11.md` §2
**Scope:** `claim_paths_are_inert_in_dark_build` + `xai-grok-shell` `team_managed_config` (~30)

---

## Status: **green** for this cluster

| Filter | Result |
|--------|--------|
| `xai-grok-config` `claim_paths_are_inert_in_dark_build` | **ok** (was red: `!verification_active()` with armed prod `v1`) |
| `xai-grok-config` `claim_tests::*` (3) | **ok** |
| `xai-grok-config` `with_dark_forces_keyless_verification_inactive` | **ok** |
| `xai-grok-config` `bump_rollback_floor_*` dark + armed | **ok** |
| `xai-grok-shell --test team_managed_config` (50) | **50 passed** |

---

## Root cause (product contracts unchanged)

Product builds embed the prod `v1` pubkey. `verification_active()` is **true by default** (`signed_policy.rs`). That is intended.

Two test suites still assumed the old **keyless / dark** shape without forcing dark:

1. **`claim_paths_are_inert_in_dark_build`** asserted `!verification_active()` with no `test_seam::with_dark` wrap (siblings already use the seam).
2. **`team_managed_config` integration suite** mocks **unsigned** deployment-config bodies and comments ("verification is inactive here"). With verification armed, `apply_fetched` returns `SignatureRejected` and nothing is persisted → mass instant fails.

Also: Cargo already documents `xai-grok-config` feature `test-support` for integration tests to inject keys / force dark, and shell enables that feature, but **`test_seam` was only gated on `cfg(all(test, debug_assertions))`**. Integration tests compile the config crate as a dependency (`cfg(test)` false), so the seam was **not available** to `team_managed_config` until the feature gate was wired.

**No product behavior change.** Armed default stays armed. Dark remains a test-only override (debug builds; release still compiles the seam out).

---

## Fix (surgical)

### 1. Wire `test-support` into the signing test seam

**File:** `crates/codegen/xai-grok-config/src/signed_policy.rs`

- `test_seam` and `with_embedded_keys` override path now:

  `#[cfg(all(any(test, feature = "test-support"), debug_assertions))]`

- Release builds still strip the seam (`debug_assertions` required).
- Matches Cargo.toml comment: shell/integration tests inject throwaway keys or force dark via `test-support`.

### 2. Dark-contract unit test uses the seam

**File:** `crates/codegen/xai-grok-config/src/managed_cache/claim_tests.rs`

- Wrap `claim_paths_are_inert_in_dark_build` body in `test_seam::with_dark(|| { ... })` (same pattern as `bump_rollback_floor_is_inert_when_dark` / signed_policy dark tests).

### 3. `team_managed_config` suite forces process-global dark

**File:** `crates/codegen/xai-grok-shell/tests/team_managed_config.rs`

- Module docs: suite is **dark (keyless)**; unsigned mocks are intentional; armed/signed contracts live in config unit tests.
- `test_home()` init calls:

  `xai_grok_config::signed_policy::test_seam::set_embedded_keys(Some(&[]))`

  then asserts `!verification_active()`.

- Process-global override (not thread-local `with_dark`) so tokio worker threads used by async `sync` / gate paths stay dark.
- Suite is already `#[serial]` + one process-global `GROK_HOME`.

### 4. Bootstrap fail-closed test made offline after tamper

**Same file:** `bootstrap_fails_closed_when_managed_policy_compromised`

- After deleting `requirements.toml`, point deployment-config URL at **5xx** (same shape as `managed_policy_gate_fails_closed_on_deleted_policy_offline`).
- Without that, a live mock re-served the policy during bootstrap (requirements restored → gate green). That only became visible once dark made initial `sync()` succeed; when armed, the test failed earlier at unsigned `sync().expect(...)`.

---

## TDD log

| Step | Evidence |
|------|----------|
| **Red** | `claim_paths_are_inert_in_dark_build`: `assertion failed: !verification_active()` |
| **Red (sample)** | Armed sync rejects unsigned mocks (cluster analysis + product `apply_fetched` path) |
| **Green** | claim_paths ok; full `team_managed_config` 50/50 ok |

---

## Commands run

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-config --lib claim_paths
nice -n 19 ionice -c3 cargo test -p xai-grok-config --lib managed_cache::tests::claim_tests
nice -n 19 ionice -c3 cargo test -p xai-grok-config --lib with_dark_forces
nice -n 19 ionice -c3 cargo test -p xai-grok-config --lib bump_rollback_floor
nice -n 19 ionice -c3 cargo test -p xai-grok-shell --test team_managed_config -- --test-threads=1
nice -n 19 ionice -c3 cargo fmt -p xai-grok-config -p xai-grok-shell
nice -n 19 ionice -c3 cargo clippy -p xai-grok-config -- -D warnings   # lib only: clean
```

`cargo clippy -p xai-grok-config --all-targets -- -D warnings` still hits **pre-existing** `std::fs::canonicalize` disallows in `managed_text/tests.rs` (untouched). Not introduced by this change.

---

## FORK / AGENTS honesty

- **Product contract unchanged:** verification stays armed with embedded `v1`.
- **No FORK/AGENTS dual-pin required:** only hermetic test seams + one offline bootstrap fixture.
- Cargo `test-support` feature for config was already documented; code now matches that comment.

---

## Residual (out of this cluster)

- Possible product follow-up (not claimed fixed): something during bootstrap still re-syncs managed config when the deployment-config mock remains live (settings-only path is supposed not to). Offline 5xx fixture is the correct fail-closed test; investigating an unexpected heal path is separate if still real under armed production.
- Other CI 239 slices (external auth, pager, worktree, oneshots) remain outside this report.

---

## Files touched

1. `crates/codegen/xai-grok-config/src/signed_policy.rs` — seam cfg for `test-support`
2. `crates/codegen/xai-grok-config/src/managed_cache/claim_tests.rs` — `with_dark` wrap
3. `crates/codegen/xai-grok-shell/tests/team_managed_config.rs` — suite dark init + offline bootstrap

No git add/commit/push.
