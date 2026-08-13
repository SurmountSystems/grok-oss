# Fix: rustc 1.97 team-managed / dark claim cluster

**Date:** 2026-08-12
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 implementer
**Prior report (same root, not in this tree):** `.agents/reports/bug-dark-signed-policy-cluster-2026-08-11.md`

---

## Status: **green**

| Filter | Result |
|--------|--------|
| `xai-grok-config` `claim_paths_are_inert_in_dark_build` | **ok** (was red: `!verification_active()`) |
| `xai-grok-config` `claim_tests::*` (3) | **ok** |
| `xai-grok-config` `with_dark_forces_keyless_verification_inactive` | **ok** |
| `xai-grok-config` `bump_rollback_floor_*` dark + armed | **ok** |
| `xai-grok-shell --test team_managed_config` (50) | **50 passed** |
| `cargo clippy -p xai-grok-config --lib -- -D warnings` | **ok** |

Not 30 unique product bugs. Shared setup: tests assumed a keyless (dark) client while the product default is armed with the compiled-in `v1` pubkey.

---

## Root cause

Product builds embed the prod `v1` pubkey. `verification_active()` is true by default. That is intended.

Two suites still assumed the old keyless / dark shape without forcing dark:

1. **`claim_paths_are_inert_in_dark_build`** asserted `!verification_active()` with no `test_seam::with_dark` wrap. Sibling dark tests already use the seam.
2. **`team_managed_config`** mocks **unsigned** deployment-config bodies (comment: "verification is inactive here"). With verification armed, `apply_fetched` returns `SignatureRejected` and nothing is persisted. Every sync-then-assert test fails in tens of milliseconds.

Also: Cargo already documents `xai-grok-config` feature `test-support` so integration tests can inject keys or force dark. The seam itself was only gated on `cfg(all(test, debug_assertions))`. Integration tests compile the config crate as a dependency (`cfg(test)` is false), so the seam was not available to `team_managed_config` until the feature gate was wired.

**No product behavior change.** Armed default stays armed. Dark remains a test-only override (debug builds; release still compiles the seam out).

The 2026-08-11 mop described this exact fix, but the three files were not in this tree when this 1.97 cluster was re-run.

---

## Fix (surgical)

### 1. Wire `test-support` into the signing test seam

**File:** `crates/codegen/xai-grok-config/src/signed_policy.rs`

- `test_seam` and `with_embedded_keys` override path now:

  `#[cfg(all(any(test, feature = "test-support"), debug_assertions))]`

- Release builds still strip the seam (`debug_assertions` required).
- Matches Cargo.toml: shell/integration tests inject throwaway keys or force dark via `test-support`.

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
- Without that, a live mock re-serves the policy during bootstrap (requirements restored, gate green). That only becomes visible once dark makes the initial `sync()` succeed.

---

## TDD log

| Step | Evidence |
|------|----------|
| **Red** | `claim_paths_are_inert_in_dark_build`: `assertion failed: !crate::signed_policy::verification_active()` at `claim_tests.rs:74` (ran with `--nocapture` before the wrap) |
| **Red (cluster)** | Armed `apply_fetched` rejects unsigned mocks (`SignatureRejected`); CI listed ~30 `team_managed_config` tests failing in 50–70 ms. Same setup, not 30 product bugs. |
| **Green** | claim_paths ok; `claim_tests::*` 3/3; full `team_managed_config` 50/50 ok |

Tests were not weakened. The dark tests still assert the dark contract. They now actually run dark.

---

## Commands run

```bash
cargo fmt -p xai-grok-config -p xai-grok-shell
cargo test -p xai-grok-config --lib managed_cache::tests::claim_tests::claim_paths_are_inert_in_dark_build -- --nocapture
# red before wrap; green after

cargo test --offline -p xai-grok-config --lib managed_cache::tests::claim_tests
# 3 passed

cargo test --offline -p xai-grok-config --lib with_dark_forces_keyless_verification_inactive
cargo test --offline -p xai-grok-config --lib bump_rollback_floor

cargo test --offline -p xai-grok-shell --test team_managed_config -- --test-threads=1
# 50 passed; 0 failed; finished in 1.49s

cargo clippy --offline -p xai-grok-config --lib -- -D warnings
# ok
```

---

## Files touched

1. `crates/codegen/xai-grok-config/src/signed_policy.rs` — seam cfg for `test-support`
2. `crates/codegen/xai-grok-config/src/managed_cache/claim_tests.rs` — `with_dark` wrap
3. `crates/codegen/xai-grok-shell/tests/team_managed_config.rs` — suite dark init + offline bootstrap

No git add/commit/push.

---

## Residual

- Product contract unchanged: verification stays armed with embedded `v1`.
- No FORK/AGENTS dual-pin required: hermetic test seams + one offline bootstrap fixture.
- Other rustc 1.97 / CI 239 slices (pager, worktree, export_github, oneshots) are outside this report.
