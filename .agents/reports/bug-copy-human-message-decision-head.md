DECISION: RESPAWN_IMPLEMENTER

Summary (15 lines)
1. Explore is present: bug-copy-human-message-explore.md (14547 bytes, Aug 15 13:10).
2. A same-size copy exists as bug-copy-human-message-explore-READY.md (also 13:10).
3. Required impl report bug-copy-human-message-impl.md is missing (test -f and ls both say no).
4. Sidecar bug-copy-human-message-impl-TIMEOUT.md exists (45740 bytes, Aug 15 13:31).
5. That timeout note waited 20 minutes; the impl path never appeared.
6. No cargo, rustc, rustup, or grok-build-target compile process is running.
7. Cache /home/hunter/.cache/grok-build-target last listing time is Aug 15 13:15; idle now.
8. Status conclusion text is died_no_report: implementer timed out or stopped without a report.
9. Explore says the bubble copy glyph is paint-only: no hit rect, no hover, no click-to-copy.
10. Suggested product fix is hit-test plus Action::CopyBlockContent on the painted icon.
11. The implementer track is not live, so IMPLEMENTER_STILL_RUNNING does not apply.
12. IMPL_REPORT_READY does not apply because the required impl.md path is absent.
13. EXPLORE_ONLY does not apply: an implementer was started and died without a report.
14. UNKNOWN does not apply: named files and process scan agree.
15. Decision: respawn the copy-human-message implementer.

===== FULL L2 INPUT =====
L2_INPUT
===== STATUS =====
# Status: copy human message (2026-08-15)

Snapshot of `/home/hunter/Projects/surmount/grok-build/.agents/reports/` after `ls -la`. No crates were touched. No subagents were spawned.

## Named-file checks

| Path | Exists |
|------|--------|
