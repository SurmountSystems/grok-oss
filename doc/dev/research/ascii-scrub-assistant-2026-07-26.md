# ASCII scrub of assistant AI text

Date: 2026-07-26 · Wave 0 S0–S4

## What

Assistant model prose is scrubbed to ASCII-safe punctuation for terminals and
downstream tools that mishandle curly quotes, em dashes, and invisible spaces.

| Class | Source | Result |
|-------|--------|--------|
| Em dash | U+2014 | `--` |
| En dash | U+2013 | `-` |
| Smart doubles | U+201C U+201D | `"` |
| Smart singles | U+2018 U+2019 | `'` |
| Zero-width / format | U+200B–D, U+2060, U+FEFF | stripped |
| Space-like | U+00A0, U+202F, U+2007–A, U+205F | ASCII space |

Not scrubbed: user messages, tool args/results, reasoning/Thought channel.

## Placement

| Layer | Path |
|-------|------|
| Pure map | `xai-grok-tools` `util/ascii_scrub.rs` |
| Shell wire + enablement | `xai-grok-shell` `session/helpers/assistant_ascii_scrub.rs` |
| Stream Text | `handle_sampling_event` (`tool_calls.rs`) |
| Chat state | `record_assistant_response` (`sampler_turn.rs`) |
| Fallback one-shot | `turn.rs` AgentMessageChunk |
| Config | `[ui] scrub_ascii_punct` on `UiConfig` (default ON) |
| Settings | Appearance **ASCII-safe assistant punctuation** |

## Enablement (default ON)

| Layer | Key | Notes |
|-------|-----|--------|
| Env | `GROK_SCRUB_ASCII_PUNCT` | unset → on; `0`/`false`/`off`/`no`/`n` → off |
| Config | `[ui] scrub_ascii_punct` | `None` → on; seeded at spawn; reloader live-updates preference |
| Settings modal | same key | persists via `Effect::PersistSetting` |
| Session override | agent approval only | see S3 |

Any of env / config / session-override off → passthrough.

## S3 — agent override with approval

Agents must not silently disable hygiene.

```
agent calls disable_ascii_scrub tool
  → shell intercept (never YOLO / Read auto-allow)
  → scrub_disable_acp_permission_options()
  → session/request_permission (AllowOnce / AllowAlways / Reject)
  → approval_from_permission_response / approval_from_permission_option
  → apply_agent_scrub_disable_request_product
       AllowAlways → set_scrub_ascii_punct(false) disk write
```

| Decision | Effect |
|----------|--------|
| `None` (cancel) / `Reject` | Scrub stays on |
| `AllowOnce` | Session AtomicBool override off for this process |
| `AllowAlways` | Session override + process pref + **disk** `[ui] scrub_ascii_punct = false` |

Stable option ids: `scrub-disable-allow-once`, `scrub-disable-allow-always`,
`scrub-disable-reject`. Map also accepts generic ACP-style ids/kinds
(`allow-once`, `allow_always`, `reject_once`, …) fail-closed to Reject.

Product entry points:
- Tool: `GrokBuild:disable_ascii_scrub` (default toolset + `ensure_plan_mode_tools`)
- Shell: `request_agent_scrub_disable` / `apply_agent_scrub_disable_request_product`
- Wire: `tool_calls.rs` intercept before normal permission manager

`seed_from_effective_config` clears the session override so a new session
starts clean. Config reloader updates the durable preference without clearing
an active session override.

## Tests

```bash
cargo test -p xai-grok-tools --lib -- ascii_scrub
cargo test -p xai-grok-shared --lib -- scrub_ascii_punct
cargo test -p xai-grok-shell --lib -- assistant_ascii_scrub
cargo test -p xai-grok-shell --lib -- channel_token_text_scrubs channel_token_text_preserves
cargo test -p xai-grok-pager --lib -- scrub_ascii
# settings e2e (integration):
cargo test -p xai-grok-pager --test settings_e2e -- scrub_ascii
```

## Docs

- User-guide: `crates/codegen/xai-grok-pager/docs/user-guide/05-configuration.md`
  § ASCII-safe assistant punctuation + env table
- FORK product bullet
- RESIDUAL §2c closed for Wave 0 scrub

## Non-goals / residual

- Thought/reasoning channel scrub
- Fence-aware keep-unicode-in-code-blocks
- Subagent-specific override scope beyond process AtomicBool
- Appearance cache / settings UI live refresh after agent AllowAlways (disk + process pref are written; UI row may need reopen)
