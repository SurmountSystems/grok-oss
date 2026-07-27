# ULID helper (workstream B0)

Date: 2026-07-25

## What

Small Crockford-base32 ULID mint/parse helpers for **new** work / log / tool
artifacts (join keys, usage-row work ids, etc.).

| Item | Location |
|------|----------|
| API | `xai_grok_tools::util::ulid` — `mint`, `parse`, `is_valid`, `normalize` |
| Dep | workspace `ulid = "1"` → `ulid` 1.2.x |
| Tests | unit tests in same module (length, charset, uniqueness, sortability-ish, roundtrip) |

## Policy

- **New** artifact / work / log ids → prefer `util::ulid::mint()` (26-char, time-sortable prefix).
- **Existing** task / tool-call / session UUID **v7** sites → leave alone; no mass rewrite.
- String identity newtypes (`SessionId`, …) already accept any scheme; ULID is one option.

## Not in this change

No call-site migration, no usage-log wiring, no task-id conversion.
