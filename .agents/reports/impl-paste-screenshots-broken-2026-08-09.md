# Fix: paste screenshots as real image attachments (2026-08-09)

Board: `bug:paste-screenshots-broken`

## Problem

Operator: "Pasting screenshots stopped working." Evidence: paste of a PNG
screenshot showed a **metadata card** (`Format: PNG · Dimensions · Size · Path`)
and `Image #N` chrome, with concern that pixels never reached the model.

## Root cause

On **Linux (and any non-macOS/non-Windows host)**, the agent Prompt
`Event::Paste` arm never deferred a clipboard **image** probe:

| Path | Behavior before |
|------|-----------------|
| Ctrl+V as **key event** | `handle_paste_key_deferred` → probe → image chip (worked) |
| Ctrl+V as **bracketed paste** (`Event::Paste`) | path drops only; **no** `ProbeClipboardAttachment` |
| Pure image clipboard (empty paste text) | nothing attached |

Many Wayland/Linux terminals deliver Ctrl+V as bracketed paste, not a key
event. Screenshot "Copy to Clipboard" often puts **only** `image/png` (no
path text). Result: paste looked dead or path-only.

Path pastes (`file://…` / bare path) already loaded bytes and showed the
metadata overlay when the terminal has no graphics protocol (`show_pixels`
false). That overlay is normal UI, not the regression; pure clipboard
image was.

Dashboard already probed on every OS. Agent Prompt was macOS/Windows only
since open-source publish. Otty IME origin gating already lives in the
off-thread probe and does not require skipping Linux enqueue.

## Fix

1. **Agent Prompt `Event::Paste`:** always run
   `bracketed_paste_should_probe` + `attachment_probe_gate` and enqueue
   `ProbeClipboardAttachment` with `BracketedInserted` (same as macOS/Windows).
2. **`route_popup_paste`** (plan soft-park, permission followup, question
   input, plan approval): same probe so plan screenshots work via
   bracketed paste, not only Ctrl+V key.
3. **`bracketed_paste_should_probe`:** available on every OS (removed cfg).

Path / `file://` drops still win first (no double-attach of file-icon raster).

## Tests (red/green contract)

| Test | Contract |
|------|----------|
| `agent_empty_bracketed_paste_defers_probe_for_clipboard_image` | Empty `Event::Paste` + raster → enqueue probe |
| `agent_bracketed_paste_stamps_ctx_bracketed` | All OS (was macOS/Windows-only) |
| `agent_path_paste_png_attaches_bytes_for_send` | Path with spaces → chip + non-empty `ContentBlock::Image` |

```text
cargo test -p xai-grok-pager --lib paste_key_tests
# 79 passed
```

## Verify

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings   # green
cargo test -p xai-grok-pager --lib paste_key_tests
```

(`--all-targets` clippy has pre-existing unrelated test-only lints; not
introduced here.)

## Files

- `crates/codegen/xai-grok-pager/src/app/agent_view/input.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/paste.rs`

## Note on metadata card

`Format: / Dimensions: / Path:` in the image preview overlay is the
**no-pixels** branch when the terminal lacks Kitty/iTerm2 graphics or
preview bytes are not ready. Attachment for the model uses
`load_for_send` / `ContentBlock::Image` independently of that UI.
Path paste was already producing sendable bytes; pure clipboard image
on Linux bracketed paste was not.

No git commit (agent policy).
