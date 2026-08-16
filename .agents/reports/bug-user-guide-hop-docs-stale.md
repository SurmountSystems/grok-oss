# User-guide hop docs match restored dual-auth hop

**Date:** 2026-08-13
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Board:** `bug:user-guide-hop-docs-stale`

SuperGrok is a paid product. This report says **included SuperGrok period limits**, never "free SuperGrok."

## Named contract

User-guide pages `02-authentication.md` and `04-slash-commands.md` must not say automatic host hop after included SuperGrok period limits are full is unshipped. Dual-auth hop is restored in source (`sampling_config_for_model` copies `failover_api_keys`; ModelsManager / prepare / run_loop / subagent override use `resolve_credentials_preferring_with_rank`).

Docs must match that source:

- Stay on SuperGrok while included SuperGrok period limits have room.
- After they are full, SuperGrok dollar credits, then console failover.
- When SuperGrok dollar credits are known positive, SuperGrok stays primary and the console key is failover.
- When they are zero or unknown, the console API key leads.

Surgical docs only. No hop Rust. No wire-key renames.

## Red evidence

No existing include/assert encoded "hop not shipped." Added `docs::tests::user_guide_does_not_claim_automatic_host_hop_is_unshipped` first (embedded `USER_GUIDE` `include_str!` copies). Then ran it against the still-stale pages.

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-hop-docs-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR"
cargo test -p xai-grok-pager --offline --lib -- \
  user_guide_does_not_claim_automatic_host_hop_is_unshipped -- --nocapture
```

**Exit code: 101**

```
test docs::tests::user_guide_does_not_claim_automatic_host_hop_is_unshipped ... FAILED

02-authentication.md still claims automatic host hop after included SuperGrok period limits are full is unshipped

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 8868 filtered out
```

Cold isolated target compiled first (several minutes). The fail above is the observed red after that compile.

## Files

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/docs.rs` | New include/assert: no user-guide page claims hop is unshipped; `02` and `04` describe hop after included SuperGrok period limits are full; no "free SuperGrok" on those two pages. |
| `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md` | Replaced the "not a shipped automatic hop" sentence with the restored hop order (extras-positive vs extras zero/unknown). |
| `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md` | Replaced "Automatic host hop ... is **not** shipped on this restack" with the same hop-after-full sentence. |
| `crates/codegen/xai-grok-pager/docs/user-guide/24-monitoring-usage.md` | Same stale "Automatic hop ... is **not** shipped" sentence. Now matches the shipped hop so the USER_GUIDE-wide stale scan stays green. |

## Green

Same isolated env. Same test, then the `user_guide` filter.

```
cargo test -p xai-grok-pager --offline --lib -- \
  user_guide_does_not_claim_automatic_host_hop_is_unshipped -- --nocapture
```

**Exit code: 0**

```
test docs::tests::user_guide_does_not_claim_automatic_host_hop_is_unshipped ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8868 filtered out
```

```
cargo test -p xai-grok-pager --offline --lib -- user_guide -- --nocapture
```

**Exit code: 0**

```
test docs::tests::user_guide_entries_are_valid ... ok
test docs::tests::user_guide_entries_have_no_duplicates ... ok
test docs::tests::default_howto_entries_includes_all_user_guide_docs ... ok
test docs::tests::user_guide_does_not_claim_automatic_host_hop_is_unshipped ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 8865 filtered out
```

```
cargo fmt -p xai-grok-pager -- --check
```

**Exit code: 0**

Did not crate-wide fmt. Did not clippy the whole pager crate (docs + one test only).

## Leftovers

- Host extract under `~/.grok/docs/user-guide/` updates on next TUI start (`extract_user_guide_docs`). Not written here.
- `FORK.md` / residual / doctor strings were out of scope. This slice is user-guide + the include/assert.
- Live TUI still needs a rebuild/install to show extracted copies. No `/rebuild`.
- Did not rewrite hop Rust.

No product hop behavior was invented. Spend order and extras-positive vs extras zero/unknown follow the existing resolve comments and the hop restore report.
