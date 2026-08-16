# Image-strip edit check

Scout: L3. Read-only. Snapshot local time: 2026-08-15 17:55:18 -0600.
Cutoff used for “newer than”: `2026-08-15 16:20:00` local (`date -d` epoch `1786832400`). File mtimes are also local `-0600`.

No product edits. No git.

## 1. Recovery report

Path: `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-poisoned-image-session-recovery.md`

| Field | Value |
|-------|--------|
| Size | 9004 bytes (`wc -c`; `stat` size=9004) |
| Lines | 129 (`wc -l`) |
| mtime | 2026-08-15 16:42:41.987247820 -0600 (epoch 1786833761) |

Exact string search (`rg -F`):

| Needle | Present |
|--------|---------|
| `New fork seam` | **no** (exit 1) |
| `## Green` | **no** (exit 1) |
| `Status: GREEN` | **no** (exit 1) |

What is on disk instead:

- Line 3: `- Status: RED (claim closed; no product edit)`
- Headings: `# Poisoned image session recovery`, `## Named contract`, `## Red evidence`, `## Test file + function`, `## Related product files / functions (no edits)`, `## Diagnosis`, `## What this L3 did not change`. No `## Green`.
- Only `GREEN` mention is line 129: “record GREEN on this file” (instruction to a later writer, not a green status).

## 2. Product / test file mtimes

| Path | Size | mtime (local) | Epoch | Newer than 2026-08-15 16:20? |
|------|------|---------------|-------|------------------------------|
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/spawn.rs` | 116603 | 2026-08-12 18:18:21.257845707 -0600 | 1786580301 | **no** |
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/model_switch.rs` | 17572 | 2026-08-12 17:56:32.850081153 -0600 | 1786578992 | **no** |
| `crates/codegen/xai-grok-shell/src/session/acp_session.rs` | 108904 | 2026-08-12 17:41:56.281993218 -0600 | 1786578116 | **no** |
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/image_strip.rs` | 5680 | 2026-08-12 17:41:56.281993218 -0600 | 1786578116 | **no** |
| `crates/codegen/xai-grok-shell/tests/test_image_strip_recovery.rs` | 7108 | 2026-08-15 17:31:32.887052154 -0600 | 1786836692 | **yes** |
| `crates/codegen/xai-grok-shell/src/agent/config.rs` | 266848 | 2026-08-14 16:28:59.013540237 -0600 | 1786746539 | **no** |

**Any of these newer than 2026-08-15 16:20?** Yes: only `tests/test_image_strip_recovery.rs` (mtime 17:31:32, about 71 minutes after the cutoff). The four session sources and `config.rs` are all older (Aug 12 or Aug 14).

Note on the test file (stat, not asked as a claim): birth/ctime-access birth is 2026-08-12 17:41:56; content mtime is 2026-08-15 17:31:32.

The recovery report itself is also newer than 16:20 (16:42:41), but it is not in the product/test list above.

## 3. Live cargo / rustc for xai-grok-shell

**Yes. Live compile of `xai-grok-shell` was in progress at snapshot.**

`pgrep -x rustc` and `pgrep -x cargo` were empty. The live jobs use `cargo-clippy`, `cargo check`, and `clippy-driver` (rustc argv, comm is not `rustc`).

Live at 2026-08-15 17:55:18 -0600:

| PID | Started (etime ~) | Command |
|-----|-------------------|---------|
| 2800399 | Sat Aug 15 17:54:25 2026 (~00:51) | `cargo-clippy clippy -p xai-grok-shell --lib -- -D warnings` |
| 2800417 | Sat Aug 15 17:54:26 2026 (~00:51) | `cargo check -p xai-grok-shell --lib` |
| 2800535 | Sat Aug 15 17:54:37 2026 (~00:39) | `clippy-driver` → `rustc --crate-name xai_grok_shell` on `crates/codegen/xai-grok-shell/src/lib.rs` (`CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`) |

Wrapping shells (also live):

- PID 2800210 (started 17:54:19): wait loop on `pgrep -x rustc` / `pgrep -x cargo`, then planned `cargo clippy -p xai-grok-shell --all-targets` and `cargo test -p xai-grok-shell --lib seeded_test_model_keeps_chat_completions_backend`.
- PID 2800262 (started 17:54:20): `cargo fmt -p xai-grok-shell` then `cargo clippy -p xai-grok-shell --lib` then `cargo test -p xai-grok-shell --lib keep_unverified_persisted_model_keeps_seeded_custom_slug`. This is the parent of the live `cargo-clippy` / `cargo check` pair.

Scout did not wait for those jobs and did not kill them.
