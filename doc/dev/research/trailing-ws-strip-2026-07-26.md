# Trailing-whitespace strip after product file edits

Date: 2026-07-26 · Crate: `xai-grok-tools`

## Behavior

After a successful **text** file edit/write via product tools, trailing
spaces and tabs are stripped from each line **before** bytes hit disk.

| Property | Value |
|----------|--------|
| Default | **ON** when env unset |
| Env | `GROK_STRIP_TRAILING_WHITESPACE` |
| Disable | `0`, `false`, `off`, `no`, `n` (case-insensitive) |
| Enable | `1`, `true`, `on`, `yes`, `y` (or other non-empty) |
| Scope | Per-line trailing spaces/tabs only |
| Line endings | Preserves `\n` vs `\r\n` |
| Final newline | Preserves whether input ended with a newline |
| Binary | Skipped via `util/binary::is_binary` (null bytes, etc.) |

## Shared helper

`crates/codegen/xai-grok-tools/src/util/trailing_ws.rs`

- `strip_trailing_whitespace(text) -> String`
- `strip_enabled() -> bool` / `should_strip(Option<bool>)`
- `prepare_for_write(text) -> String` — call at write sites

## Wired tools

| Tool | Path |
|------|------|
| `GrokBuild:search_replace` (+ concise) | create + replace before `fs.write_file` |
| OpenCode `write` | before write |
| OpenCode `edit` | create + replace before write |
| Codex `apply_patch` | on Add / Update / Move derived content |
| Hashline edit | after `apply_edits`, before write |

Not stripped: shell/`run_terminal_command` out-of-band writes; host editor
format-on-save; agent prompt markdown that is not a file-edit tool.

## Override surfaces (v1)

1. **Env** (above) — sufficient for ops and tests.
2. **Tool arg** — deferred; `prepare_for_write_with_override` exists for a
   future optional `strip_trailing_whitespace: Option<bool>` on schemas
   without a multi-crate construction churn in this batch.

## Tests

- Unit: `util::trailing_ws::tests::*`
- Integration: `search_replace` strips / env-off preserves / CRLF preserve;
  OpenCode `write` strips / env-off preserves

```bash
cargo test -p xai-grok-tools --lib -- trailing_ws strips_trailing search_replace::tests::strips
```

## Non-goals

- Full formatters (rustfmt)
- Host editor format-on-save
- Markdown hard-break two spaces (use env off when intentional)
- Stripping inside `LocalFs::write_file` (binary-safe raw path)
