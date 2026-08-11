# Prefer Rust tools over ad-hoc Python/bash

**Date:** 2026-07-26
**Class:** D2 research brief (not D1 law; not a diary).
**Raw inventories (scratch):** `/tmp/grok-1000/python-inventory-product.md`,
`python-inventory-host.md`, `rust-tool-surface-vs-python.md`.

## Goal

Prefer **embedded Rust** (compiled into the agent/product binary) over ad-hoc
Python/bash. Surfaces should feel like **first-class tools**, but the critical
path is **invisible intercept**: when the model still uses
`run_terminal_command` / `python3 …/known-script.py`, the terminal tool
**hot-wires** that call to the embedded implementation (ACP-like protocol
surface → internal function), not “go install this bin and remember the name.”

Optional: also expose real tool names for discovery. External CLI bins alone
are **not** the architecture. User-project Python still shells for real.

Aims: less interpreter surface, secure hot paths, token-efficient (no
training the model to shell `python3` for our jobs).
## Inventory summary

| Layer | What | Count / note |
|-------|------|----------------|
| **Product `*.py`** | encrypt templates, example recursive-grep hook, memory ACP harness | **3** only |
| PyO3 / embed | — | **0** |
| Default agent binary needs python | — | **No** |
| CI `pkgs.python3` | ci-tools (cgroup + mock LSP tests) | Intentional hermetic dep today |
| Host orchestration | `memory.py`, `validate-plan.py` | **P0/P1** every implement / plan dry-run |
| Host hooks | `block-bulk-replace-edit.py` | **P0** every edit |
| Host resume | `session_reader.py`, legacy `cc_session.py` | **P1** untrusted transcripts |
| Office / PDF skills | ~45 unique office + 8 PDF (+ 3× office trees) | **P3** — keep py / system bins long-term |
| Rust tool surface | read/grep/search_replace/bash/todos/subagents/MCP/web/memory_search… | Strong; **GAP**: jq-like JSON tool, native bulk-edit policy bin |

## Product vs host hot paths

| Priority | Path | Why rewrite |
|----------|------|-------------|
| **P0** | `~/.agents/skills/implement/scripts/memory.py` | Every implement/execute-plan; workspace memory file (0o600) |
| **P0** | `~/.grok/hooks/block-bulk-replace-edit.py` | **A3 product embed done** (`util/bulk_edit_policy` on search_replace); host hook dual-pin still for shell/matcher |
| **P0** | Product **steer text** (`QueryTools` + consumers) | **Done (A1)** — no longer names `python3`; dump/overflow keep `jq`/`sed`/`cut`; confusable edits → re-read only |
| **P1** | `validate-plan.py`, `session_reader.py` | **A4 shipped:** plan_validate full; session_reader Claude + Codex SQLite/rollout + Cursor CLI/desktop SQLite |
| **H (product maint)** | `encrypt_templates.py` | XOR already mirrored in Rust; small codegen |
| **H (product opt)** | `no-recursive-grep-guard.py` example | Security-shaped; needs host python when installed |
| **M** | `run_tests.py`, cgroup/`python3 -c`, LSP mock py | Dev/CI only; drop CI python3 after ports |
| **P3** | office/pdf skill scripts | Ecosystem (openpyxl, pypdf, soffice) — last or never full port |

## Agent steer problem

Product probes PATH for `python3`/`python` and **steers** recovery/dump/edit
hints toward shell Python (`query_tools.rs` → use_tool dump, MCP truncate,
web_fetch overflow, search_replace hints, read_file docs). That burns tokens
and widens shell blast radius when **native tools already cover** read/grep/edit.

**Fix class:** rewrite steers to `read_file` / `grep` / `search_replace` /
artifact paths; optional later JSON-slice tool. Do **not** ban user-project
Python in auto_mode.

**A1 landed (product text only):** `util/query_tools.rs` dropped `python`
field; `json_tools` = `jq` only; `text_tools` = `sed`/`cut`; removed
`edit_tools` + confusable shell-script terminal fallback (native re-read +
shorter `old_string` only). Consumers: MCP dump / `use_tool`, web_fetch
overflow, `read_file` long-line hint, `search_replace` confusable.

**A2 landed (embed + intercept):** in-process
`util/implement_memory` ports host `memory.py` (`path` / `read` / `snapshot` /
`update` with flock + 0o600). `BashTool::run` intercepts allowlisted
`python3 …/implement/scripts/memory.py …` (and echo-pipe / `< file` update
forms) and never spawns Python for those; user-project `python3 foo.py`
still shells. Optional first-class tool name not registered this pass.

**A3 landed (bulk-edit policy embed):** `util/bulk_edit_policy` ports host
`block-bulk-replace-edit.py` into the product `search_replace` path (before
apply): optional `GROK_DENY_REPLACE_ALL=1`, multi-file same-hunk storm
(N=5 / T=120, state under `~/.grok/bulk-edit-state/`, fail-open I/O). Host
PreToolUse hook remains dual-pin for shell/ACP matcher surface.

## Phase plan

| # | Phase | Status |
|---|--------|--------|
| 1 | Inventory + hierarchical pins (this brief + D0/D1 links) | **Done** |
| 2 | Embedded Rust handlers + first-class tools + **terminal intercept** (P0: implement-memory, bulk-edit; demote steers) | **Partial** — A1 steers; **A2 implement-memory**; **A3 bulk-edit**; **A4 plan_validate full + session_reader full**; **skill-text demotion** (host intercept notes + review/zed no python3 heredoc); optional first-class tool names + host py drop still open |
| 3 | Widen intercept catalog / telemetry; optional CLI only as thin entry to same code | Open |
| 4 | Delete py where safe (stdlib helpers); office/PDF last; drop CI `python3` only if tests no longer need it | Open |

## Top rewrite order

| Rank | Candidate | Effort | Outcome |
|------|-----------|--------|---------|
| 1 | Steer demotion (python3 → native tools) | S–M | **Done (A1):** `QueryTools` no longer probes/names python; confusable edit steers drop shell-script fallback; dump/overflow/read steers keep `jq`/`sed`/`cut` only. A2 = embed+intercept later. |
| 2 | `memory.py` → embed + bash intercept (+ tests) | M | **Done (A2):** `util/implement_memory` + `BashTool` intercept; host implement/execute-plan skill text notes intercept; keep allowlisted CLI for dual-pin |
| 3 | `block-bulk-replace-edit.py` → product embed | M | **Done (A3):** `util/bulk_edit_policy` on `search_replace` (storm + optional `GROK_DENY_REPLACE_ALL`); host hook remains complementary dual-pin |
| 4 | `session_reader.py` (+ retire `cc_session.py`) | M | **Done (A4 SQLite):** `util/session_reader` + bash intercept; Claude list/show; Codex `state_*.sqlite` + rollout jsonl read; Cursor CLI `store.db` + desktop `state.vscdb` discovery/read; fixture tests; fail closed. Host resume-session CORE documents intercept; keep allowlisted CLI form. `.jsonl.zst` clear error. |
| 5 | `validate-plan.py` → embed + intercept | S–M | **Done (A4):** `util/plan_validate` full DAG parity + bash intercept |
| 6 | Skill text: review/zed `python3 <<` → `jq` / tools; intercept notes on allowlisted paths | S | **Done:** host review + zed-settings drop non-intercepted python3; resume-session / implement / execute-plan document Grok bash intercept (keep allowlisted CLI form for match + host dual-pin) |
| 7 | `encrypt_templates.py` → build.rs / cargo bin | S | Zero maintainer product py for codegen |
| 8 | Example recursive-grep guard → rust / in-process | M | First-class PreToolUse |
| 9 | Cgroup + LSP test helpers without python3 | S–M | Drop hermetic CI python3 |
| 10 | Port hot cases from `run_tests.py` → nextest | L | Reliability |
| … | Office text-replace / PDF forms | L / skip | Prefer system tools; full port last |

Phase 2 shape: **in-process handlers** (+ optional tool registration), with
`run_terminal_command` intercept for known python/skill paths. Thin `grok …`
CLI only if useful for humans — same code path, not a separate product.

## Non-goals (this campaign)

- Ban **user project** Python / pytest / uv (auto_mode + shell stay intentional)
- Wholesale ban or rewrite of **office/pdf** skills this pass
- Port LibreOffice macro recalc, full OOXML XSD stacks, reportlab recipes
- Multi-file bulk find-replace **tool** (policy forbids; intentional GAP)
- Grow D1 AGENTS with inventories — law stays short; detail lives here
- Implement encrypt/memory rewrites in the pin-only pass

## Already covered in Rust (don’t invent python)

read_file, search_replace, grep (rg), list_dir, bash + task lifecycle, todos,
subagents, MCP search/use, web_fetch/search, memory_search/get (when on),
hooks runner, secret redaction, code-graph / fast-worktree bins.

## Pin map (hierarchy)

| Layer | Where |
|-------|--------|
| **D0** | `RESIDUAL.md` Open — migration item (link here) |
| **D1** | `AGENTS.md` (product + `~/.grok`), `_SKILL_RULES` standing rule |
| **D2** | This brief; raw inventories under `/tmp/grok-1000/` |
| **FORK** | Process checkbox — preference + inventory pinned |

## Agent invent ban (process law 2026-08-09)

Separate from the embed/intercept campaign above: agents must **not write and
execute new** Python scripts or ad-hoc download/run shell scripts for agent
work (supply-chain risk on the Python ecosystem). Prefer Rust tools and named
product commands (`cargo`, `just`, tests, `rg`). Shell tool ≠ inventing a
script. Full tables and exceptions: host `~/.grok/AGENTS.md` § *Prefer Rust
tools; do not invent…*; product `AGENTS.md` hard constraint 6; skill-rules
rule 17. This brief remains the **migration inventory** (P0–P3 ports), not
a substitute for that ban.

## Related

- Hermetic CI python3: `doc/dev/research/ci-fail-30139839732-python3-hermetic-2026-07-24.md`
- FORK CI section documents intentional `python3` in ci-tools
