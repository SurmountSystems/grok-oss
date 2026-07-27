# UDAX — JSON → TOON (model-facing tool results)

Date: 2026-07-26 · tree: grok-build

## Goal

Use **JSON** as the programmatic source of truth; encode as **TOON**
([Token-Oriented Object Notation](https://github.com/toon-format/toon), SPEC
working draft) when structured data is rendered into **model context**.
ACP/MCP protocol wire stays JSON-RPC/JSON.

Tagline: *Use JSON programmatically; encode as TOON for LLM input.*

## T0 — library choice

| Option | Decision |
|--------|----------|
| A. Rust crate | **Chosen:** [`toon-format`](https://crates.io/crates/toon-format) **0.5.0** with `default-features = false` (encode/decode only; no CLI/clap/ratatui) |
| B. Embed SPEC encode | Not needed for v1; crate round-trips SPEC-ish goldens in-tree |
| C. Shell out to `@toon-format/cli` | **Rejected** — never on runtime path |

Spike: tabular uniform object arrays, primitive inline arrays, nested objects
round-trip; measurable byte win vs compact JSON on uniform arrays.

Spec pin: <https://github.com/toon-format/toon> → SPEC repo
<https://github.com/toon-format/spec> (crate docs say v3.0; SPEC tip may be
newer — golden tests guard our surface).

## T1 — API (`xai_grok_tools::util::toon`)

| Item | Detail |
|------|--------|
| `encode(&Value) -> Result<String>` | Thin wrap `toon_format::encode_default` |
| `decode(&str) -> Result<Value>` | Thin wrap `toon_format::decode_default` |
| `maybe_encode_for_llm(value, policy)` | Policy-aware model text |
| `ToolResultFormat` | `Auto` (default) \| `Toon` \| `Json` |
| Env | `GROK_TOOL_RESULT_FORMAT=toon\|json\|auto` |

**Auto heuristic:** emit TOON when `is_tabular_eligible` (uniform object arrays
or non-empty primitive arrays, shallow) **or** TOON byte length &lt; compact
JSON; else compact JSON. Encode errors fail open to compact JSON.

## T2 — tool result integration

Chokepoint: `ToolOutput::to_prompt_format()` for **`Dynamic`** (structured
`serde_json::Value` from runtime/MCP-adapter tools that land as Dynamic).

- Before any downstream size caps on the **rendered** string, densify via
  `maybe_encode_for_llm_from_env`.
- Free text (bash, read_file, ordinary `ToolOutput::Text`) **unchanged**; pure
  object/array JSON `Text` blobs densify under the same policy (**T5**).
- No ACP/MCP protocol framing changes.

## T3 — MCP densify before truncate (shipped)

Chokepoint: `util::mcp_truncate::densify_mcp_result_text` runs at the start of
`truncate_mcp_text` (MCP + Text variants on the use_tool path). Structured
JSON object/array → `maybe_encode_for_llm_from_env`; free text / invalid JSON
unchanged. Protocol envelopes untouched.

## T4 — `json_to_toon` agent tool (shipped)

| Item | Detail |
|------|--------|
| Tool id | `json_to_toon` (GrokBuild registry) |
| Input | `json`: structured value **or** JSON text string (parsed first) |
| Output | TOON text via `util::toon::encode` |
| Errors | Invalid JSON text → `ToolError::invalid_arguments` with clear message |
| Protocol | ACP/MCP envelopes **unchanged** |

## T5 — prompt / subagent handoff densify (shipped)

Shared helper: `util::toon::densify_structured_text` / `_in_place` (object/array
JSON only; free text / scalars / invalid JSON unchanged; fail-open). T3 MCP
path delegates here (single policy parser).

Model-facing wires: TaskOutput body, SubagentCompleted body, completion
inline delivery, SearchTool content, SchedulerList, pure-JSON `ToolOutput::Text`,
child subagent task prompt when pure structured JSON. On-disk
`prompt_context.json` stays loadable JSON. ACP/MCP envelopes unchanged.

## T6 — savings metrics (shipped)

Fail-open `tracing::debug!(before_bytes, after_bytes, saved_bytes, "toon densify: N_json → N_toon")`
inside densify; never fails the turn.

## Deferred (residual)

Soft only: additional model-facing JSON chokepoints if dogfood finds one.

## Validate

```bash
cargo test -p xai-grok-tools --lib -- toon json_to_toon
cargo test -p xai-grok-tools --lib -- dynamic_to_prompt free_text densify_mcp
```

## Files

- `crates/codegen/xai-grok-tools/src/util/toon/mod.rs`
- `crates/codegen/xai-grok-tools/src/types/output.rs` (`Dynamic` branch)
- `crates/codegen/xai-grok-tools/src/implementations/grok_build/json_to_toon/mod.rs`
- `crates/codegen/xai-grok-tools/Cargo.toml` (`toon-format` dep)
