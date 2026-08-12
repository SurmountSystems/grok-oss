# PATH probe sites — residual local≠CI risk (2026-07-24)

**Scope:** read-only inventory of host-`PATH` optional-tool probes that can still
skew local vs GHA **after** a hypothetical `cargo-ci` PATH scrub (deny-list or
allowlist), if product/unit code still walks the process `PATH` or tests assert
on live probe outcomes.

**Related:**
- `doc/dev/research/flake-hermeticity-path-trace-2026-07-24.md` (how PATH reaches nextest)
- `doc/dev/research/flake-hermeticity-inventory-2026-07-24.md` (impurity ranking)
- `doc/dev/research/ci-doctor-cmd-fail-2026-07-24.md` (fixed voice issue-count flake)

**Risky test asserts (total issue counts including live probes): `0`**

No remaining unit/integration assert was found that requires
`issue_count() == N` (or equivalent total findings) while also running live
host probes (`apply_voice_probe` / ambient clipboard/tmux inventory). The prior
failure mode was fixed by filtering `VOICE_NO_INPUT_DEVICE_ID` in
`doctor_cmd::tests::fake_standalone_facts_compose_through_shared_view`.

---

## Legend

| Column | Meaning |
|--------|---------|
| Kind | **product** runtime discovery vs **unit test** / **integration** harness |
| Hermetic? | **injected** seam, **fixture/fake PATH**, **host PATH**, or **opt-in ignore** |

---

## High impact (desktop optional tools → doctor / diagnostics)

| Path | Symbol | Kind | Hermetic? |
|------|--------|------|-----------|
| `crates/codegen/xai-grok-voice/src/audio/capture_linux.rs` | `binary_on_path`, `detect_recorder`, `detect_recorder_with`, `require_recorder` | product (Linux mic: `pw-record` → `parec` → `arecord`) | product: **host PATH**. Unit tests of preference order: **injected** via `detect_recorder_with` (no live PATH). |
| `crates/codegen/xai-grok-voice/src/audio/capture_linux.rs` | `input_device_info` | product | host PATH via `require_recorder` / detect |
| `crates/codegen/xai-grok-pager/src/diagnostics/mod.rs` | `apply_voice_probe` | product | live `xai_grok_voice::input_device_info()`; appends `voice.no-input-device` Issue when missing |
| `crates/codegen/xai-grok-pager/src/doctor_cmd/mod.rs` | `collect_report`, `collect_report_with` | product | always `apply_voice_probe(..., true)` after view |
| `crates/codegen/xai-grok-pager/src/app/dispatch/prompt.rs` | `collect_live_doctor_report_for_terminal` | product (TUI `/doctor`) | live voice when `voice_mode_enabled()` |
| `crates/codegen/xai-grok-shared/src/clipboard.rs` | `native_tool_name`, `linux_tool_spec` / `probe_tool_spec`, `tool_available` | product | Linux: spawn `wl-copy` / `xclip` / `xsel` `--version` via `Command::new` (PATH). macOS label is constant `"pbcopy"`. Cached `OnceLock`. |
| `crates/codegen/xai-grok-pager/src/diagnostics/probes/mod.rs` | `collect_standalone`, `collect_doctor_tui`, `collect_startup_tui` | product | live `native_tool_name()`; `collect_standalone` skips live tmux (unavailable facts); fix path uses `LiveTmuxProbe` |
| `crates/codegen/xai-grok-pager-render/src/terminal/tmux_probe.rs` | `build_tmux_command` / `run_tmux_bounded` | product | `Command::new("tmux")` on PATH |
| `crates/codegen/xai-grok-pager/src/diagnostics/probes/tmux.rs` | `LiveTmuxProbe` | product | wraps tmux_probe |
| `crates/codegen/xai-grok-pager-render/src/clipboard/mod.rs` | tmux load-buffer / set-buffer paths | product | `Command::new("tmux")` |

### Doctor / voice unit tests (composition)

| Path | Symbol | Kind | Hermetic? |
|------|--------|------|-----------|
| `…/doctor_cmd/tests.rs` | `fake_standalone_facts_compose_through_shared_view` | unit | Fake snapshot via `collect_standalone_from`; **still runs live** `apply_voice_probe` through `collect_report_with`. Asserts **view issues only**, excluding `VOICE_NO_INPUT_DEVICE_ID` — **hermeticized assert** (2026-07-24). |
| `…/doctor_cmd/tests.rs` | `standalone_wayland_missing_is_issue_but_no_seats_or_errors_are_not` | unit | Fake snapshot + live voice; asserts **specific** `wayland-data-control` ID, not total count |
| `…/doctor_cmd/tests.rs` | `standalone_runtime_and_tmux_are_unavailable_without_false_wezterm_finding` | unit | Same pattern; asserts absence of named IDs / probe note lists, not totals |
| `…/doctor_cmd/tests.rs` | `clipboard_issue_count_*`, JSON count fixtures | unit | Pure `healthy_report()` fixtures — **no live PATH** |
| `…/diagnostics/doctor_format_tests.rs` | `issue_count()` asserts | unit | Fixture snapshots only — **no live PATH** |
| `…/diagnostics/mod.rs` | `voice_missing_finding_has_stable_id_and_manual_remediation` | unit | Builds finding from string — **no live PATH** |
| `…/voice/…/capture_linux.rs` | `recorder_preference_is_pipewire_then_pulse_then_alsa` | unit | **Injected** `detect_recorder_with` |

### Integration / harness (PATH often controlled)

| Path | Symbol | Kind | Hermetic? |
|------|--------|------|-----------|
| `…/pager/tests/doctor_early_dispatch.rs` | doctor fix / list tests | integration (`#[ignore]` needs `PAGER_BINARY`) | Most inject **fake PATH** with stub `tmux`. `base_pager_command` defaults to parent PATH when not overridden; `env_clear` otherwise. |
| `…/pager/tests/pty_e2e/middle_click_pastes_primary_linux.rs` | primary paste | integration `#[ignore]` | Prepends fake `xclip`/`xsel` on PATH — **fixture PATH** |
| `…/pager-pty-harness/src/host_clipboard.rs` | `pbcopy` / `pbpaste` | harness / macOS bench | Real host tools; benches/e2e only, not default nextest gate |
| `…/pager-pty-harness/src/pty.rs` | mux env scrub comments | harness | Scrubs `TMUX` for child isolation (env, not binary PATH) |
| `…/tmux_probe.rs` unit | timeout / fake bin tests | unit | Serialized PATH inject with fake `tmux` binary |

---

## Medium impact (optional CLI discovery; usually soft)

| Path | Symbol | Kind | Hermetic? |
|------|--------|------|-----------|
| `crates/codegen/xai-grok-config/src/shell.rs` | `is_command_available` (`which::which`) | product | host PATH |
| same | `unix_shell_path` cascade (`which::which` + fixed dirs) | product | host PATH + `/bin`… fallbacks |
| same tests | `is_command_available_detects_present_and_absent` | unit | Probes `"sh"` / bogus name — soft (sh expected everywhere CI cares about) |
| `crates/codegen/xai-grok-pager/src/inline_media_ffmpeg.rs` | `ffmpeg_available`, package-manager probe | product | host PATH; tests use **`set_ffmpeg_available_for_test` / install-cmd inject** |
| `crates/codegen/xai-grok-pager/src/wrap_cmd.rs` | wrap target `is_command_available` | product | host PATH |
| `crates/codegen/xai-grok-tools/src/util/query_tools.rs` | `QueryTools::detect` (`jq`/`python`/`sed`/`cut`) | product | host PATH; steers only (no hard fail assert in unit tests found) |
| `crates/codegen/xai-grok-tools/…/web_fetch/error.rs` | `gh_available` | product | host PATH; unit uses **`which_in` on temp dir** |
| `crates/codegen/xai-grok-tools/…/embedded_search_tools.rs` | `which::which` + shell `command -v` for bfs/ugrep | product | host PATH for hints; unit tests assert inject string shape / skip if no bash |
| `crates/codegen/xai-grok-tools/…/shell_state.rs` | `which::which("ugrep")` in test | unit | **Conditional** assert only when ugrep present |
| `crates/codegen/xai-grok-mermaid/src/mmdc.rs` | `detect_mmdc` / `MmdcEngine::detect` | product optional | host PATH; no default auto-select |
| `crates/codegen/xai-grok-mcp/src/servers.rs` | `which::which` / `which_in` for server bins | product | host PATH for MCP server resolution |
| `crates/codegen/xai-grok-shell/src/extensions/pr.rs` (+ update/gh call sites) | `Command::new("gh")` | product | host PATH when feature used |
| `crates/common/xai-test-utils/src/git.rs` | `ensure_hermetic_git_on_path` | test util | **Prepends** hermetic git when `GIT_BIN_PATH` set (Bazel); does not scrub optional desktop tools |

---

## Low / non-flake for quality gate

| Path | Notes |
|------|--------|
| Install scripts (`xai-grok-pager/scripts/install*.sh`) | `command -v curl/wget` — packaging, not nextest |
| `permission/shell_access.rs` | Parses `command -v cat` as shell form; does not spawn host tools |
| Hardcoded `/bin` `/usr/bin` shell fallbacks | Intentional host-dev; not optional desktop inventory |
| Crane pure builds | Separate sandbox PATH; not GHA quality cargo path |

---

## What a cargo-ci PATH scrub would still miss

1. **In-process unit tests** that call `collect_report_with` / product APIs still use the **test process** PATH (scrub helps if cargo-ci sets it for nextest children; bare `cargo test` remains impure).
2. **Product** live discovery is intentional for real users (mic, clipboard CLI, tmux, ffmpeg). Do not “fix” product by always finding tools.
3. **Injected seams already exist** where it mattered for gates: voice preference (`detect_recorder_with`), ffmpeg test guards, doctor snapshot constructors, fake PATH for tmux/xclip integration tests.
4. **Memoized** clipboard `linux_tool_spec` / ffmpeg OnceLock: first probe under impure PATH sticks for process lifetime if scrub is incomplete mid-suite.

---

## Recommendation (tests only)

- **No further change required** for total-issue-count asserts including live probes (**count = 0** remaining).
- If more composition tests call `collect_report_with`, keep asserting **named finding IDs** or exclude `VOICE_NO_INPUT_DEVICE_ID` (and any future live ambient findings), never raw `issue_count()` against a pure fixture expectation.
- Optional hygiene: inject/disable voice probe in `collect_report_with` under `#[cfg(test)]` so composition tests do not touch host audio PATH at all — product binary path unchanged.

---

## Count summary

| Metric | Value |
|--------|-------|
| **Risky test asserts** (total issue counts ⊇ live probes) | **0** |
| Prior fixed assert | 1 (`fake_standalone_facts…` / `assert_eq!(report.issue_count(), 1)`) |
| Product PATH probe families still ambient | voice recorders, Linux clipboard CLI, tmux, ffmpeg, jq/python/sed, gh, mmdc, MCP bins, shell resolve |
