# Fork docs: image-strip session-restore seam pin

- Status: pinned (docs only)
- Date: 2026-08-15
- Role: L3 docs finisher. No product `*.rs`. No new cargo tests.
- Source: `.agents/reports/bug-poisoned-image-session-recovery.md` (`## Product change`, New fork seam: yes)

`session/load` keeps a seeded custom model id on Chat Completions instead of remapping it to the default grok-4.5 Responses catalog entry. grok-4.5 itself still uses Responses. SuperGrok is paid. This is not last-session on start.

This write does **not** claim live TUI dogfood and does **not** invent cargo.

## Files touched

- [`FORK.md`](../../FORK.md)
  - Product extras one-liner after `from_config` no-prefetch usable catalog
  - Extra proven restack-droppable class (not a new numbered land class)
  - Operator cheat sheet extra cargo block
- [`doc/dev/upstream-regression-filters.md`](../../doc/dev/upstream-regression-filters.md)
  - Extra restack-droppable neighbor section with three rows
  - Other high-value extras mention
  - Operator cheat sheet extra cargo lines

Not touched: product `*.rs`, user-guide pages, `RESIDUAL.md`, `just` recipes, cargo tests.

## Exact names enrolled

Verified with `rg` for `fn` before listing. All three exist.

| Name | Where the `fn` lives | How enrolled |
|------|----------------------|--------------|
| `keep_unverified_persisted_model_keeps_seeded_custom_slug` | `xai-grok-shell` `agent/models/tests.rs` | FORK Product + extras + cheat sheet; catalog extra row + cheat sheet |
| `seeded_test_model_keeps_chat_completions_backend` | `xai-grok-shell` `agent/mvp_agent/tests.rs` | same |
| `poisoned_image_session_recovers_within_the_failing_turn` | `xai-grok-shell` `--test test_image_strip_recovery` | same (integration; in-turn strip after 400 `invalid_image`) |

Existing cargo shapes only:

```
cargo test -p xai-grok-shell --lib -- keep_unverified_persisted_model_keeps_seeded_custom_slug \
  seeded_test_model_keeps_chat_completions_backend
cargo test -p xai-grok-shell --test test_image_strip_recovery -- \
  poisoned_image_session_recovers_within_the_failing_turn
```

## What stayed out

Do not treat these as enrolled land identifiers.

| Left out | Why |
|----------|-----|
| A new seventh or eighth numbered land class | Last-session on start already owns opening the remembered session versus Welcome. It does not own seeded-model restore. This seam is Product extras only. |
| Last-session on start filters (`materialize_new_auto_*`) | Different contract. |
| `invalid_image_code_strips_and_retries` | Pre-existing sampler strip-retry. Not this new restore seam. |
| Product functions `keep_unverified_persisted_model`, `restore_persisted_model`, `resolve_sampling_config_for_model`, `model_entry_for_apply` | Implementation names, not cargo `fn`s. |
| Helpers `poisoned_image_data_uri`, `seed_poisoned_session`, `chat_completion_bodies`, `session_chat_jsonl` | Not tests. |
| Sampler / shell persist rewrite as a new land class | The implementer did not rewrite those paths. |
| Pager crate, clippy crate noise, empty `models_cache.json` miss | UNPROVEN or unrelated. |
| Live TUI / rebuilt `grok-oss` dogfood | Not claimed. |
| grok-4.5 leaving Responses | Not claimed. grok-4.5 still uses Responses. |

No git.
