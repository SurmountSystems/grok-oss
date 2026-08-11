# Implement: real `/rebuild` progress bar

**Date:** 2026-08-08
**Crates:** `xai-grok-update`, `xai-grok-pager`, `xai-grok-pager-bin`
**Prior mid-build capture fix:** `.agents/reports/impl-rebuild-tui-glitch-mid-2026-08-08.md` (kept)

---

## UX before / after

| Before | After |
|--------|--------|
| Static toast "Rebuilding grok-oss..." then occasional stage toast lines (`Rebuild: ==> ...`, `Compiling ...`) | Full-width **progress strip** at the bottom of scrollback: `Rebuild [████░░░░]  42%  Compiling xai-grok-pager (12 packages)` |
| No overall fraction; stages felt like a spinner with text | Weighted overall fraction `0..=100%` driven by pipeline stages + real crate compile counts |
| CLI `grok-oss rebuild` printed stage lines only | CLI rewrites a single stderr bar line: `[████░░░░░░░░]  42%  detail` |
| Capture-only (no inherit) already fixed footer glitch | Capture **unchanged** (still no cargo on the TUI PTY) |

Operator note: TUI mid-rebuild glitch is treated as fixed (capture path); this work only upgrades progress UX.

---

## How real progress is computed

### Weighted pipeline (`rebuild_progress_weights`)

| Segment | Overall fraction |
|---------|------------------|
| Resolve source tree | 0.02 |
| Install start (`just install` / cargo start) | 0.05 |
| Cargo compile | 0.05 → 0.88 |
| Strip | 0.91 |
| Install binary | 0.95 |
| Verify | 0.97 |
| Soft-relaunch leaders | 0.99 |
| Done | 1.00 |

### Cargo segment (real counts, not wall-clock)

1. **just install path:** parse human `Compiling <crate>` lines from captured just/cargo output. Each unique crate name increments the count.
2. **cargo fixed-argv fallback:** `cargo build ... --message-format=json` → parse `compiler-artifact` package names and `build-finished`.
3. Soft denominator: `compiled / (compiled + 8)`, capped at 0.98 of the cargo window until `Finished` / `build-finished` snaps to cargo end (0.88 overall).
4. Engine is **monotonic**: `advance_to` never decreases fraction.

### API

```text
RebuildProgressEvent { fraction: f32, detail: String }
RebuildProgressEngine  // pure ingest of stage/cargo lines
format_rebuild_cli_progress(fraction, detail, bar_width)
```

Callback path: `rebuild_and_relaunch_with_progress` → install worker emits events over a channel → async drain → TUI/CLI. Leaders/done emit after install.

---

## TUI wiring

| Piece | Role |
|-------|------|
| `AgentView.rebuild_progress: Option<RebuildUiProgress>` | Live bar state |
| `TaskResult::RebuildProgress { message, fraction }` | Channel → dispatch |
| `views/rebuild_progress.rs` | Pure strip layout (reuses `progress_bar_tracked_spans`) |
| `agent_view/render.rs` | Paints strip at bottom of scrollback while rebuild is live |
| On fail / after success summary | Clears `rebuild_progress` |

Capture / no-inherit contracts from the mid-build fix remain green.

---

## Tests (red → green contracts)

### `xai-grok-update` (`cargo test -p xai-grok-update --lib rebuild::`)

18 passed, including:

- `install_stdio_policy_is_always_capture` (capture kept)
- sanitize / stage-filter contracts (kept)
- `rebuild_fraction_clamped_0_to_1`
- `rebuild_progress_engine_is_monotonic_across_stages`
- `cargo_artifact_messages_drive_detail_and_fraction`
- `cli_progress_bar_includes_blocks_percent_and_detail`
- `rebuild_progress_bar_chars_reflects_fraction`
- `cargo_sub_fraction_uses_counts_not_time`

### `xai-grok-pager`

- `rebuild_progress_updates_bar_and_toast_not_scrollback`
- `rebuild_progress_clamps_fraction_on_dispatch`
- `views::rebuild_progress::*` (bar glyphs, percent, stage, clamp)
- Existing rebuild relaunch / capture-related tests still green

### Commands

```text
cargo test -p xai-grok-update --lib rebuild::
# exit 0 — 18 passed

cargo test -p xai-grok-pager --lib rebuild_
# exit 0 — includes bar dispatch + render helpers

cargo fmt -p xai-grok-update -p xai-grok-pager -p xai-grok-pager-bin
# exit 0

cargo clippy -p xai-grok-update --lib --all-targets -- -D warnings
# exit 0

cargo clippy -p xai-grok-pager --lib -- -D warnings
# exit 0
```

---

## Install / dogfood binary

```text
just install
# exit 0

~/.cargo/bin/grok-oss --version
# grok-oss 0.2.111 (c87f66a61d94) [stable]
```

Path: `/home/hunter/.cargo/bin/grok-oss` (stripped).

---

## Files touched

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-update/src/rebuild.rs` | Progress engine, weights, cargo parse, structured callback, tests |
| `crates/codegen/xai-grok-update/src/lib.rs` | Re-exports |
| `crates/codegen/xai-grok-pager/src/views/rebuild_progress.rs` | **New** pure strip + unit tests |
| `crates/codegen/xai-grok-pager/src/views/mod.rs` | Module |
| `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs` | `RebuildUiProgress` + field |
| `crates/codegen/xai-grok-pager/src/app/agent_view/session.rs` | Init |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Paint strip |
| `crates/codegen/xai-grok-pager/src/app/actions.rs` | `RebuildProgress.fraction` |
| `crates/codegen/xai-grok-pager/src/app/effects/helpers.rs` | `RestoreProgressMsg.fraction` |
| `crates/codegen/xai-grok-pager/src/app/effects/mod.rs` | Structured progress send |
| `crates/codegen/xai-grok-pager/src/app/event_loop.rs` | Map fraction |
| `crates/codegen/xai-grok-pager/src/app/dispatch/task_result.rs` | Store bar + toast |
| `crates/codegen/xai-grok-pager/src/app/dispatch/rebuild.rs` | Clear strip on done/fail |
| `crates/codegen/xai-grok-pager/src/app/dispatch/router.rs` | Seed 0% on start |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/task_result.rs` | Bar contracts |
| `crates/codegen/xai-grok-pager-bin/src/main.rs` | CLI stderr progress bar |

No git commit / stage (agent policy).

---

## Remaining limits (honest)

| Limit | Notes |
|-------|--------|
| just install cargo has no JSON | Relies on human `Compiling` lines; soft denominator until `Finished` |
| Soft cargo denominator | Not a known total package count; bar approaches 0.88, then stage markers finish the job |
| justfile steps without cargo markers | Coarse jumps only (strip / install / verify echoes) |
| Live multipane paint | Unit tests cover pure render + dispatch; operator dogfood for full alt-screen |
| Toast also updated | Long-lived toast mirrors percent + stage as a fallback when the strip row is tight |

---

## Operator check

1. Run installed `grok-oss`.
2. `/rebuild` from a live session.
3. Mid-build: full-width `Rebuild [███░...] NN% Compiling ...` strip; footer/composer intact (capture).
4. CLI: `grok-oss rebuild` shows a rewriting bar on stderr.
5. After relaunch: new binary as today.
