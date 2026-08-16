# Report: Token Economy spend order on Configuration

**Date:** 2026-08-14
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Scope:** surgical user-guide leftover only. No Rust product edits. No git add / commit / push.

## What changed

`crates/codegen/xai-grok-pager/docs/user-guide/05-configuration.md` Token Economy section now matches pages 02, 04, and 24.

The section already named SuperGrok as paid and distinguished included SuperGrok period limits from SuperGrok dollar credits. It did not state the four-step spend order (Business / Team included first, then personal included, then SuperGrok dollar credits that never expire, then console). One paragraph was added after that intro. The knobs table, implement-effort application order, pacing, and double-entry notes were left as they were.

New copy (chrome and rank):

1. Spend included SuperGrok period limits on stored Business / Team SuperGrok logins first.
2. Then personal included SuperGrok period limits.
3. Then SuperGrok dollar credits that never expire.
4. Then console team prepaid / console API credits.
5. Remaining included SuperGrok period limits across distinct stored plans are added together. That sum is the real remaining included quota. A unified pool counts once.
6. SuperGrok is paid (already in the first sentence). Operator CLI is `grok-oss limits` / `grok-oss limits --json`.

Not invented on this page: grok.com account switcher, a second OAuth login story, daemon / SIGUSR1 snapshot notes. Those stay on Authentication and Slash Commands.

## Tests

`user_guide_names_token_economy_spend_order` still only includes `02-authentication.md` and `04-slash-commands.md`. This pass did not change `docs.rs`.

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-te-ug05-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-pager --lib -- user_guide_names_token_economy_spend_order user_guide_operator_cli_examples_use_grok_oss -- --test-threads=1
```

Result: `ok. 2 passed; 0 failed` (8876 filtered out).

## Files

- Edited: `crates/codegen/xai-grok-pager/docs/user-guide/05-configuration.md` (Token Economy section only)
- Report: this file
