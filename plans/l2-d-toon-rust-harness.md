# Plan 0 — TOON-first, Rust-first harness

> Make agent-facing structured output TOON by default, and replace the one-off Python and bash scripts agents still invent with tested Rust harness tools. Keep JSON only where an external API or a named `--json` machine client requires it. Do not reinvent `util/toon`.

**North star:** an agent looking at tool results, skill-helper stdout, path probes, JSON slices, and session `usage.jsonl` sees TOON unless the wire is someone else's JSON. The five most common one-off script patterns become named Rust tools with failing tests first, then product, then docs.

**Baseline (what exists today):**
- Shared encode/decode already ships: `xai_grok_tools::util::toon` (`crates/codegen/xai-grok-tools/src/util/toon/mod.rs:1-14`, `59-67`). Env `GROK_TOOL_RESULT_FORMAT=auto|toon|json` (`mod.rs:21-24`, `51-56`). Unset env is `ToolResultFormat::Auto` (`mod.rs:51-56`, test `env_policy_default_auto` at `mod.rs:405-408`). `#[default]` on the enum is `Auto` (`mod.rs:27-31`).
- Auto still emits compact JSON when the value is not tabular-eligible and TOON is not shorter (`mod.rs:135-138`). That is the live agent-facing default, not TOON-always.
- Dynamic / structured text already densify through that policy: `ToolOutput::Dynamic` and pure-JSON `Text` (`crates/codegen/xai-grok-tools/src/types/output.rs:1000-1005`); SchedulerList (`output.rs:987-995`); SearchTool content (`output.rs:868`); MCP truncate densify (`crates/codegen/xai-grok-tools/src/util/mcp_truncate.rs:198-211`). Research pin: `doc/dev/research/udax-json-toon-2026-07-26.md:8-12`, `T0-T6`. Residual: `RESIDUAL.md:326-335`. FORK checkbox: `FORK.md:139-144`.
- `json_to_toon` agent tool already exists (`crates/codegen/xai-grok-tools/src/implementations/grok_build/json_to_toon/mod.rs:14-29`, `67-69`). Do not reinvent it.
- On-disk and protocol JSON that must stay JSON: `prompt_context.json` (`crates/codegen/xai-grok-shell/src/session/acp_session.rs:1404-1405`, research `udax-json-toon-2026-07-26.md:80`); MCP HTTP probe `Content-Type: application/json` (`crates/codegen/xai-grok-mcp/src/servers.rs:2415-2421`); CLI `--json` printers (`running_sessions.rs:91-92,108-112`; `limits_cmd.rs:1051-1068`; `xai-grok-pager-bin/src/main.rs:2018-2026`).
- MCP bodies that already fit the inline cap skip densify so quoted JSON stays (`mcp_truncate.rs:217-218`). Inventory L3 must decide whether default-TOON overrides that skip.
- YAML is not the tool-result default. YAML is agent-definition frontmatter (`crates/codegen/xai-grok-agent/src/config.rs:1231-1232`) and compaction service config (`crates/common/xai-grok-compaction/src/intra_compaction/config.rs:50-54`). No agent-facing YAML dump chokepoint was found in `to_prompt_format`.
- One-off Python/bash: grok-oss already refuses non-allowlisted `python3` (`crates/codegen/xai-grok-tools/src/implementations/grok_build/bash/no_python3.rs:1-5,21-24`; report `/home/hunter/.agents/reports/fix-no-python3-in-grok-oss-2026-09-21.md` EXISTS). Allowlisted `memory.py` / `validate-plan.py` / `session_reader.py` intercept to Rust (`FORK.md:2233-2241`). Agents still teach and run those CLI forms, still pretty-print JSON from them, and still invent `test -f` loops, `jq`, and `usage.jsonl` parsers. Frequency sketch (this turn, reports only):

```
rg -l --glob '*.md' 'test -f' /home/hunter/.agents/reports | wc -l     -> 2968
rg -l --glob '*.md' 'memory\.py' /home/hunter/.agents/reports | wc -l  -> 287
rg -l --glob '*.md' 'usage\.jsonl' /home/hunter/.agents/reports | wc -l -> 263
rg -l --glob '*.md' 'limits --json' /home/hunter/.agents/reports | wc -l -> 93
rg -l --glob '*.md' '`jq`| jq |\| jq' /home/hunter/.agents/reports | wc -l -> 26
rg -l --glob '*.md' 'validate-plan\.py' /home/hunter/.agents/reports | wc -l -> 17
rg -l --glob '*.md' 'session_reader\.py' /home/hunter/.agents/reports | wc -l -> 14
rg -l --glob '*.md' 'python3 -c' /home/hunter/.agents/reports | wc -l   -> 7
```

Cited examples (each `test -f` EXISTS): `/home/hunter/.agents/reports/impl-running-memory-flush.md:16` (`python3 …/memory.py update < …json`); `/home/hunter/.agents/reports/impl-memory-flush-seven-slices.md:12`; `/home/hunter/.agents/reports/impl-three-layer-implement-helpers.md:94,124`; `/home/hunter/.agents/reports/impl-killall-forensic-fix-2026-08-08.md:100` (`python3 -c "import json,sys; d=json.load…"`); `/home/hunter/.agents/reports/ask-stock-skills-python.md:128` (bundled review `python3 <<'PY'` heredoc); `/home/hunter/.agents/reports/s4-just-check-remote-s4-current-2026-08-19.md:99-110` (`nix profile list --json | jq`). Host scripts still on disk: `/home/hunter/.agents/skills/implement/scripts/memory.py` EXISTS; `/home/hunter/.agents/skills/execute-plan/scripts/validate-plan.py` EXISTS; `/home/hunter/.agents/skills/shared/resume-session/session_reader.py` EXISTS. Research inventories under `/tmp/grok-1000/python-inventory-*.md` are MISSING. Session `chat_history.jsonl` hits for `python3` were 26072 files; that count is not one-off frequency (AGENTS.md copies). Do not treat it as a rank.

---

## Goals

### G1 — Default agent-facing format is TOON
Change unset `GROK_TOOL_RESULT_FORMAT` and `ToolResultFormat` default from `Auto` to `Toon` in `util/toon/mod.rs` (lines 7-8, 21-23, 27-31, 51-56, 127-144). Keep `Auto` as an explicit policy token. Keep `Json` for `GROK_TOOL_RESULT_FORMAT=json` and for external APIs. Reuse `encode` / `maybe_encode_for_llm` / `densify_structured_text`. Do not add a second TOON crate.

Later implementers touch at least:
- `crates/codegen/xai-grok-tools/src/util/toon/mod.rs` (default + tests)
- `crates/codegen/xai-grok-tools/src/types/output.rs` (comments + Dynamic/Text tests around `output.rs:1000-1005`, `3145-3167`)
- `crates/codegen/xai-grok-tools/src/util/mcp_truncate.rs` (the already-fits skip at `217-218`; decide TOON vs keep-small-JSON with a named test)
- Pretty-JSON stdout that never hits densify today: `util/implement_memory/ops.rs:94-97`; `util/plan_validate/mod.rs:9,43-50`; `util/session_reader/mod.rs:91-103,116-121`; `implementations/grok_build/implement_memory/mod.rs:24-25,58-65`; `implementations/grok_build/plan_validate/mod.rs:26,42-54`; `implementations/search_tool/mod.rs:259-265`; `implementations/use_tool/mod.rs:112,127`
- Docs: `doc/dev/research/udax-json-toon-2026-07-26.md` (T1 default row); FORK `FORK.md:139-144`; user-guide currently has no `GROK_TOOL_RESULT_FORMAT` hit (grep this turn: no matches under `**/user-guide/**/*.md`)

Leave CLI `--json` and protocol envelopes as JSON: `limits_cmd.rs:1051-1068`, `running_sessions.rs:91-112`, `pager-bin/src/main.rs:2018-2026`, MCP `servers.rs:2415-2421`, `prompt_context.json` (`acp_session.rs:1404`).

### G2 — Top 5 one-off patterns become Rust harness tools
Rank by this-turn report frequency, then whether a first-class TOON tool already exists. Prefer wrapping existing `util/*` over new Python or bash.

| Rank | Pattern (evidence) | Tool | Crate / surface | Why this, not a new script |
|---|---|---|---|---|
| 1 | `test -f` loops (2968 report files) | `path_probe` | `xai-grok-tools` GrokBuild tool + rustdoc | Batch exists/missing/type as TOON. Agents are required to `test -f`; a named tool replaces the bash for-loop. |
| 2 | `usage.jsonl` inspection (263 files) | `session_usage` | **new** module under `xai-grok-tools` (read JSONL, print TOON). Do **not** edit `token_economy/ledger.rs` or `l2_token_tracking.rs` | Agents parse JSONL with python/jq. L2-C owns the token report tool and SQLite ledger. This tool is a file reader, not the ledger. |
| 3 | `jq` / `python3 -c json.load` (26 + 7 files; GAP in `doc/dev/research/python-to-rust-tools-2026-07-26.md:35`; steer still names `jq` in `query_tools.rs:9-13,48-49`) | `json_query` | `xai-grok-tools` GrokBuild tool; JSON Pointer over `serde_json::Value`; TOON out via `util::toon::encode` | No `json_query` / `json_slice` symbol exists in-tree (grep this turn: no matches). |
| 4 | `python3 …/memory.py snapshot\|update` (287 files; pretty JSON at `implement_memory/ops.rs:94`) | `implement_memory` (complete the port) | existing tool `IMPLEMENT_MEMORY_TOOL_NAME` (`implementations/grok_build/implement_memory/mod.rs:17`) | Intercept already Rust. Snapshot stdout is still pretty JSON. Densify through `maybe_encode_for_llm_from_env`. Keep intercept. Do not ship new Python. |
| 5 | `python3 …/session_reader.py … --json` (14 files; pretty JSON at `session_reader/mod.rs:91-103`; intercept-only, no GrokBuild tool) | `session_reader` first-class tool | new `implementations/grok_build/session_reader/` wrapping `util::session_reader`; default agent-facing TOON; `--json` / `json: true` stays JSON for host parity | Completes the A4 port (`python-to-rust-tools-2026-07-26.md:93-94`). |

Adjacent, not in the top 5: `plan_validate` pretty JSON (`plan_validate/mod.rs:9,43-50`) should densify in the same default-TOON wave because the chokepoint is the same class, but frequency is 17. `limits --json` (93 files) stays JSON because it is a named machine flag (`limits_cmd.rs:1051-1058`). `encrypt_templates.py` (14 files) is maintainer codegen, not an agent harness. Office/PDF Python stays an exception (`FORK.md:2242-2243`).

### G3 — TDD: named failing tests first
Named tests are contracts. Later L3s write these **red** before product edits. Cargo/rustc/`just check-remote` were **not sampled** this turn (forbidden on this L3).

**Default TOON (red while Default is Auto):**
- `env_policy_default_toon` in `util/toon/mod.rs` (unset env == `ToolResultFormat::Toon`; replaces the assert in `env_policy_default_auto` at `mod.rs:405-408`)
- `dynamic_to_prompt_default_is_toon_for_nontabular` in `types/output.rs` (nested object that is not tabular-eligible must not start with `{` under unset env)
- `maybe_encode_default_policy_is_toon` (nontabular object encodes as TOON keys, not compact JSON)

Keep existing `maybe_encode_json_policy_is_compact_json` (`mod.rs:368-378`) and `dynamic_to_prompt_json_policy_is_compact_json` (`output.rs:3145-3167`). Keep `Auto` tests when policy is explicitly `auto`.

**JSON only where an external API requires it (red if TOON leaks onto the wire):**
- `mcp_http_probe_content_type_stays_application_json` (covers `servers.rs:2415-2421`)
- `prompt_context_filename_stays_json` (covers `acp_session.rs:1404`; on-disk loadable JSON)
- `limits_cli_json_flag_emits_json` (`limits_cmd.rs:1051-1068`; `--json` still pretty JSON)
- `running_cli_json_flag_emits_json` (`running_sessions.rs:91-112`)
- `version_json_flag_emits_json` (`pager-bin/src/main.rs:2021-2026`)
- `mcp_already_fits_policy_named` (explicit decision on `mcp_truncate.rs:217-218`, not a silent skip)

**Per top-5 tool (each has tests and docs):**
- `path_probe_missing_is_missing_toon`, `path_probe_existing_file_toon`, `path_probe_output_is_not_json_object_start`
- `session_usage_reads_fixture_jsonl_toon`, `session_usage_null_output_tokens_not_printed_as_zero` (does not open `ledger.rs`)
- `json_query_pointer_toon`, `json_query_does_not_invoke_jq`, `json_query_invalid_pointer_errors`
- `implement_memory_snapshot_stdout_is_toon` (plus existing `implement_memory_snapshot_intercept_does_not_spawn_shell` in `bash/mod.rs:6500`)
- `session_reader_tool_list_is_toon`, `session_reader_json_flag_still_json_for_host_parity`

Then product (green: same tests pass), then docs (user-guide + rustdoc + FORK one-liner for the default change). Do not rewrite a failing assert to match old Auto behavior.

### G4 — Write set, conflicts, and layers
This Plan 0 writes only `plans/l2-d-toon-rust-harness.md`. Later L3s own inventory, tests, implement, and one review.

**Allowed later write set (disjoint from other L2s):** `crates/codegen/xai-grok-tools/src/util/toon/mod.rs`, `types/output.rs`, `util/mcp_truncate.rs`, `util/implement_memory/**`, `util/plan_validate/**`, `util/session_reader/**`, `implementations/grok_build/json_to_toon/**`, `implementations/grok_build/implement_memory/**`, `implementations/grok_build/plan_validate/**`, new `implementations/grok_build/path_probe/`, `json_query/`, `session_usage/`, `session_reader/`, `crates/codegen/xai-grok-tools/src/registry/types.rs` (register new tools), `query_tools.rs` (stop steering `jq` once `json_query` exists), research/FORK/user-guide docs that name the default.

**Forbidden / conflict (name the path, do not edit):**
- L2-A: subagent dock visibility (do not touch that chrome)
- L2-B: AtomicU64 counters and SQLite token schema (`crates/codegen/xai-grok-shell/src/token_economy/ledger.rs`, `grok_oss.db` ingest)
- L2-C: the token report tool itself (`crates/codegen/xai-grok-pager/src/app/agent_view/l2_token_tracking.rs`, `slash/commands/what.rs`, `model_call_cost_for_sessions` in `ledger.rs:656`)
- `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs` (EXISTS; do not edit)
- If a later edit would share one of those files, stop and report the conflict. `session_usage` must be a new tools-crate reader, not a ledger change.

L2 coordinates. L3 implements. There is no L4. No new Python. No throwaway bash as the ported tool.

## Workstreams
| # | Work | Done when |
|---|---|---|
| W0 | This Plan 0 | File on disk at `plans/l2-d-toon-rust-harness.md`; format matches `/home/hunter/0_session_bus_toon.md` (EXISTS this turn) |
| W1 | Inventory L3 | Cited file:line for every JSON/YAML agent-facing default and every one-off example; `test -f` first; MISSING if absent; top 5 rank confirmed or revised with new counts |
| W2 | Tests L3 (red) | Named tests above exist and fail for the right reason (default still Auto; tools missing; JSON stdout). Do not fit tests to code. |
| W3 | Implement L3 (green) | Default is TOON; JSON remains for named external/CLI wires; top 5 tools land with the same tests green; no new Python/bash |
| W4 | Docs | User-guide + rustdoc + FORK/research default row; each new tool has docs |
| W5 | One reviewer L3 | Single review of W1-W4; not a second implementer |

Tests in Rust (`xai-grok-tools` lib tests, plus pager/MCP tests for `--json` / `application/json` wires). No Python drivers.

## Success criteria
1. Inventory is cited (file:line and report/log paths; MISSING where `test -f` failed).
2. Named test: unset `GROK_TOOL_RESULT_FORMAT` is TOON for agent-facing structured output.
3. Named test: JSON remains where an external API or `--json` machine client requires it (MCP JSON-RPC, `prompt_context.json`, CLI `--json`).
4. Top 5 tools exist as tested Rust with docs; agents do not need `python3`, `jq`, or a `test -f` for-loop for those jobs.
5. Reports written: this plan; inventory/implement/review reports under `~/.agents/reports/`.

## Non-goals
Reinventing `util/toon` or `json_to_toon`. Changing ACP/MCP protocol framing to TOON. Making `grok-oss limits --json` / `running --json` / `version --json` TOON. Editing `viewer.rs`, L2-A dock chrome, L2-B SQLite token schema / AtomicU64 counters, or the L2-C token report tool. Shipping new Python or throwaway bash as the port. Office/PDF skill Python. User-project Python. Running cargo/rustc/`just check-remote` on this laptop from this L3.

## What's next after Plan 0
Inventory L3 (confirm chokepoints and frequency), then tests L3 (red named tests), then implement L3 (green default + top 5), then one reviewer L3. L2-D coordinates those four. Do not start implement before the failing tests exist.

## Inventory confirmation (2026-10-24)

Full citations: `/home/hunter/.agents/reports/l3-l2-d-inventory-2026-10-24.md` EXISTS. This section does not rewrite Plan 0. It records what Inventory L3 confirmed or revised.

**Top 5 ranks confirmed.** Method stays frequency, then whether a first-class TOON tool already exists. New counts this turn (`rg -l --glob '*.md'` under `/home/hunter/.agents/reports`): `test -f` **2970** (was 2968), `memory.py` **287**, `usage.jsonl` **265** (was 263), `limits --json` **93**, Plan 0 jq pattern **26**, `validate-plan.py` **17**, `session_reader.py` **14**, `python3 -c` **7**. Looser `\bjq\b` is **29**. `python3 …/memory.py` CLI form **235**. `encrypt_templates.py` **14**. `/tmp/grok-1000/python-inventory-*.md` still MISSING.

Caveats, not rank swaps: 2970 `test -f` files include instruction copies (`for p in` / `for f in` loops are 40). Most of the 7 `python3 -c` files teach not to invent it; one real `json.load` is `impl-killall-forensic-fix-2026-08-08.md:100`. `usage.jsonl` ∩ python/jq overlap is 6 files (example `cost-clean-residual-handoff-l3-2026-10-17.md:92`). `memory.py` 287 is higher than `usage.jsonl` 265; usage stays rank 2 because implement_memory already exists as a tool.

**Chokepoint revisions (do not skip):**

1. Pretty JSON that **does** hit densify when wrapped as `ToolOutput::Text` or `SearchTool`: `output.rs:868`, `1005`. Bash intercept stdout does **not** (`output.rs:733`, `bash/mod.rs:2112-2130`). `use_tool` pretty JSON densifies only when over the MCP cap (`use_tool/mod.rs:112,127`; skip at `mcp_truncate.rs:217-220`).
2. MCP already-fits skip: **keep** under-cap quoted JSON. Default Toon does not override that skip unless `truncate_mcp_text` is edited. Named test `mcp_already_fits_policy_named` documents keep-small-JSON.
3. Media gen `prompt_text` is compact JSON that never densifies (`output.rs:108-124`, `1006-1009`). Add G3 test `media_gen_prompt_default_is_toon` and densify in G1. Plan 0 G3 did not name this.
4. `version --json` is compact `serde_json::to_string` (`pager-bin/src/main.rs:2021-2026`), not pretty. Still JSON.
5. Child task prompt densify: residual T5 claims it; this-turn densify call sites are TaskOutput / SubagentCompleted / SearchTool / Text / MCP truncate only. Do not invent a spawn-prompt encoder.
6. New tools: prefer `registry/types.rs` `#[path]` like `json_to_toon` (`types.rs:25-29`). Do not edit `grok_build/mod.rs` (`pub mod token_report` at `mod.rs:41` is L2-C). If L2-C also registers in `registry/types.rs`, stop and report the conflict.

YAML remains frontmatter only (`config.rs:1231-1232`). `limits --json` stays JSON. `json_to_toon` is not reinvented. Host `memory.py` / `validate-plan.py` / `session_reader.py` still EXISTS. `path_probe`, `json_query`, `session_usage`, and first-class `session_reader` dirs are MISSING.
