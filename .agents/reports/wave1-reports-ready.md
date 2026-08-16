# Wave-1 reports readiness

Polled until 2026-08-15T16:05:23-06:00 (past the ~12 minute cap). Two of four named reports exist and look finished. Two never appeared.

## 1. bug-pager-selection-render-red.md

- **Status:** missing
- **Path:** `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-pager-selection-render-red.md`
- **Size:** —
- **First heading / last lines:** file does not exist

## 2. bug-poisoned-image-session-recovery.md

- **Status:** missing
- **Path:** `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-poisoned-image-session-recovery.md`
- **Size:** —
- **First heading / last lines:** file does not exist

## 3. fork-docs-finish-map.md

- **Status:** ready
- **Path:** `/home/hunter/Projects/surmount/grok-build/.agents/reports/fork-docs-finish-map.md`
- **Size:** 14699 bytes
- **First heading:** `# FORK docs finish map`
- **Last lines:**

```
- New cargo tests for UNPROVEN seams
- Product / `*.rs` edits
- Host skill / justfile / history reminder rewrites (already done)
- Wholesale user-guide em-dash scrub
- Parking any leftover above as "optional later"

End of map.
```

## 4. bug-workspace-daemon-takeover-flaky.md

- **Status:** ready
- **Path:** `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-workspace-daemon-takeover-flaky.md`
- **Size:** 3727 bytes
- **First heading:** `# Flake glance: \`take_over_declines_when_lock_is_never_released\``
- **Last lines:**

```
## Files changed

- `crates/codegen/xai-grok-workspace-daemon/src/daemonize.rs`
  - `spawn_predecessor` waits for a live `"sleep"` cmdline match.
  - New test-only helper `wait_until_process_name_matches`.

No red/green of the named cargo test locally (it was already green here). Evidence for the harden is the 2/3000 empty-cmdline probe plus the existing sibling comment, not a local fail of this test.
```

End of readiness list.
