# CI doctor_cmd fail — 2026-07-24

## Verdict

**fixed** (test hermeticity; product code unchanged)

## Failure

| Field | Value |
|-------|--------|
| Run | https://github.com/SurmountSystems/grok-oss/actions/runs/30132611987/job/89610064969 |
| Suite | nextest workspace (26692 pass / 1 fail) |
| Test | `xai-grok-pager doctor_cmd::tests::fake_standalone_facts_compose_through_shared_view` |
| Panic | `crates/codegen/xai-grok-pager/src/doctor_cmd/tests.rs:243` |
| Assertion | `assert_eq!(report.issue_count(), 1)` → **left: 2, right: 1** |

Local `just check` was green because developer PATH has `pw-record` / `parec`; headless GHA does not.

## Root cause

1. `doctor_cmd::collect_report_with` composes the shared diagnostic **view** from a fake standalone snapshot, then always calls `apply_voice_probe(&mut report, true)`.
2. On Linux, voice probe looks for `pw-record` / `parec` / `arecord` on `PATH`. CI has none → `voice.no-input-device` **Issue**.
3. Fake snapshot still correctly produces one view issue: `terminal.tmux-clipboard`.
4. `issue_count()` became **2** on CI (tmux-clipboard + voice). Locally with recorders, voice is `Ok` → count stayed **1**.
5. The test intended to pin **shared-view composition** of fake facts, not host mic inventory.

Reproduced locally by running the old assertion with a PATH that excludes recorders:

```text
findings=[(terminal.tmux-clipboard, Issue), (voice.no-input-device, Issue)]
issue_count=2
assertion `left == right` failed  left: 2  right: 1
```

## Fix

Surgical test change only:
`$REPO/crates/codegen/xai-grok-pager/src/doctor_cmd/tests.rs`

- Assert on **view issues**: `Issue` findings excluding `VOICE_NO_INPUT_DEVICE_ID`.
- Still require exactly one view issue, and that it is `terminal.tmux-clipboard`.
- Still forbid `terminal.control-mode`.

Product `apply_voice_probe` behavior in standalone `grok doctor` is intentional (report missing mic when audio is supported).

## Verification

- Target test binary, PATH excluding `pw-record`/`parec`/`arecord`: **20/20** ok
- Target test binary, full PATH (recorders present): **10/10** ok
- `cargo nextest run -p xai-grok-pager 'doctor_cmd::tests::'`: **17/17** ok
- Old assertion under recorder-free PATH: **fails with left:2** (matches GHA)

## Files changed

- `crates/codegen/xai-grok-pager/src/doctor_cmd/tests.rs`
- `doc/dev/research/ci-doctor-cmd-fail-2026-07-24.md` (this note)

## Not done

- No `git commit` / push (human-only)
- Full `just check` / workspace nextest not re-run here (scoped suite green; change is one unit test)
