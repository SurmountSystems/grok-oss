# User-guide: token-economy spend order

Docs only. No Rust product behavior changed. No git add / commit / push.

SuperGrok is paid. This report says **included SuperGrok period limits** and **SuperGrok dollar credits**.

## Pages changed

- `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md`
  - Rank preference and `/limits` spend-order now say Business / Team included first, then personal included, then SuperGrok dollar credits that never expire, then console team prepaid / console API credits.
  - Remaining included SuperGrok period limits across distinct stored plans are added together. That sum is the real remaining included quota. A unified pool counts once.
  - A second SuperGrok plan is visible only after a second `grok-oss login` that stores the Team principal. grok.com's account switcher is a different product.
  - Only one `grok-oss` process fetches billing and limits. Other live TUIs read a snapshot under `$GROK_HOME`. There is no extra daemon. Rebuild SIGUSR1 is not this.
- `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md`
  - `/login` and `/limits` carry the same second-login honesty and spend-order sentences.
  - `/limits` also names combined remaining, unified pool once, one fetcher, snapshot, no extra daemon, and Rebuild SIGUSR1 is not this.
- `crates/codegen/xai-grok-pager/docs/user-guide/24-monitoring-usage.md`
  - Existing limits / extras / hop paragraph only. Added Team-then-personal spend order, never-expiring SuperGrok dollar credits, combined remaining, then the existing hop after included SuperGrok period limits are full. No snapshot / SIGUSR1 dump on this OTEL page.

Include test (existing hop-docs pattern):

- `crates/codegen/xai-grok-pager/src/docs.rs`
- `user_guide_names_token_economy_spend_order`

Kept the existing hop include phrases so `user_guide_does_not_claim_automatic_host_hop_is_unshipped` stays green. CLI examples stay `grok-oss`. No `free SuperGrok`. No invented grok.com workspace switcher.

## Verify

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-te-ug-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo fmt -p xai-grok-pager
cargo test -p xai-grok-pager --lib -- user_guide -- --test-threads=1
```

Result: 7 passed, 0 failed. Includes `user_guide_names_token_economy_spend_order`, hop include, and `user_guide_operator_cli_examples_use_grok_oss`.

## Leftover

- `05-configuration.md` Token Economy still has the older three-meter hop copy. Out of this slice (named pages only).
- `24-monitoring-usage.md` does not repeat the one-fetcher / snapshot / second-login sentences. Those live on Authentication and Slash Commands.
- Host `~/.grok/docs` extract stays stale until the next product launch copies the embedded guide.
- Pre-existing em dashes elsewhere on those pages were not scrubbed.
- Wire JSON `activeDriver` value `supergrok_free_period` is still documented as a wire name, not operator chrome.
