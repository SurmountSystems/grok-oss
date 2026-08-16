# Implement report: honest discovered SuperGrok identities (Slice A)

**Board:** `impl:te-discover-identities`  
**Parent:** `feat:token-economy-all-plans-ipc`  
**Date:** 2026-08-14  
**Isolated compile:** rustc 1.97.1, `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-te-discover-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`

SuperGrok is paid. This report says **included SuperGrok period limits**, never "free SuperGrok."

Spend order, combined remaining, flock snapshot hub, and user-guide 02/04/24 were already done. This slice does not change rank.

No new login UX. No grok.com workspace-switcher OAuth.

---

## Operator contract

Make "what we can see" first-class and tested.

- `limits --json` and `/limits` already listed dual SuperGrok principals when both slots exist. They now also carry a combined **Discovered identities** block: each SuperGrok role plus fingerprint when known (no secrets), each console key fingerprint, and an honest single-session note when only one SuperGrok session is stored.
- Doctor still lists principals. When only one SuperGrok session is present, it adds: included SuperGrok period limits can only be checked for that login until a second `grok-oss login`.
- One stored SuperGrok session does not invent a Business / Team row.

---

## Red (observed, then product)

The two named tests were **new**. `dual_principals_stack_in_report` already listed two principals (weaker, no discovered-identities block). That existing test stayed green. The two-slot named test is a tighten, not a fake red.

Command (after types + empty field + tests, before constructors inferred discovered identities):

```bash
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-te-discover-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-pager --lib -- \
  limits_json_lists_two_supergrok_principals_when_both_slots_exist \
  limits_json_honest_single_supergrok_session_cannot_see_team_plan
```

**1. `limits_json_lists_two_supergrok_principals_when_both_slots_exist`**

- Fail: `discovered identities must list both stored SuperGrok sessions`
- `report.discovered_identities.supergrok_sessions.len()` was `0`, expected `2`
- `DiscoveredIdentities` was still default-empty (`only_one_supergrok_session: false`, `honesty: None`)

**2. `limits_json_honest_single_supergrok_session_cannot_see_team_plan`**

- Fail: `one slot must be marked as a single SuperGrok session`
- Same empty `DiscoveredIdentities` (`only_one: false`, `honesty: None`)

Doctor tests that expect `NOTE_SINGLE_SUPERGROK_SESSION_CANNOT_SEE_TEAM_PLAN` were written before `format_human` emitted the sentence. This continuation did not re-run those doctor tests while the sentence was still missing. They are green after the product sentence landed.

---

## Product

Smallest wiring after the observed red:

- `DiscoveredIdentities` / `DiscoveredSupergrokSession` on the `/limits` snapshot and `limits --json` report (`discoveredIdentities`, camelCase).
- Infer roles from principal rows when the fixture has no JWT. Live `grok limits` overlays `DiscoveredIdentities::from_dual_auth` so fingerprints come from stored sessions, not invented tokens.
- Human `/limits` prints a **Discovered identities:** section (role, optional mode, fingerprint; console key fingerprints; single-session honesty).
- Doctor `DualAuthStatus::format_human`: one sentence when a SuperGrok session is present and fewer than two principals are listed.
- Console-only `from_billing` with no SuperGrok included meter does **not** invent a SuperGrok session.

Honesty constant (exported from `xai-grok-shell`):

`Included SuperGrok period limits can only be checked for that login until a second grok-oss login.`

---

## Files changed

- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`
- `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs`
- `crates/codegen/xai-grok-shell/src/auth/mod.rs` (re-export of the honesty constant)
- `doc/dev/upstream-regression-filters.md` (2b catalog names)
- `FORK.md` (land cheat sheet §5)
- `RESIDUAL.md` (2b cargo line, keep in sync)

No rank files. No new login path.

---

## Green re-run

```bash
cargo test -p xai-grok-pager --lib -- \
  limits_json_lists_two_supergrok_principals_when_both_slots_exist \
  limits_json_honest_single_supergrok_session_cannot_see_team_plan
```

`2 passed; 0 failed`

Keep-green sample (rank / hop / hub / dual `/limits` / doctor):

```bash
cargo test -p xai-grok-pager --lib -- \
  format_dual_principals dual_principals_stack_in_report
cargo test -p xai-grok-shell --lib -- \
  format_human_single_supergrok_session_says_cannot_see_team_plan \
  dual_supergrok_principals_listed_with_fingerprints_only \
  pick_prefers_business_included_before_personal_when_both_have_remaining \
  sampling_config_hops_to_sibling_included_before_extras \
  limits_snapshot_second_process_reads_file \
  afterburner_does_not_skip_mark_when_sibling_has_included_remaining
```

All passed. Two stored SuperGrok sessions still do **not** get the single-session doctor sentence.

---

## FMT / CLIPPY / TEST exits

| Step | Command | Exit |
|------|---------|------|
| fmt | `cargo fmt -p xai-grok-pager -p xai-grok-shell` | 0 |
| fmt check | `cargo fmt -p xai-grok-pager -p xai-grok-shell -- --check` | 0 |
| clippy pager | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | 0 |
| clippy shell | `cargo clippy -p xai-grok-shell --lib -- -D warnings` | 0 |
| named tests | pager filter above | 0 |
| keep-green | pager + shell filters above | 0 |

---

## Leftovers

- TUI `/limits` still builds from `from_billing` (one SuperGrok section). Dual principal **rows** remain the CLI `from_principals` path. TUI can infer one SuperGrok role when SuperGrok is live. Fingerprints on the discovered block are filled on live `grok limits` collect (doctor listings), not in the hermetic TUI dispatch path (no extra disk I/O in that handler).
- No second `grok-oss login` UX. A Team / Business plan is visible only after that login writes a second slot.
- Slice B/C/D neighbors (combined remaining chrome, hop, hub) were already shipped. This slice did not change them.

## Next implement prompt

implement Slice leftovers only if the operator names a new slice. Auto-run should not chain: remaining token-economy work is other slices, not this honesty surface.
