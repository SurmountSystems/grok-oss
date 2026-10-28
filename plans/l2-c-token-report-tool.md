# Plan 0. Token report tool: estimate vs actual, human table or TOON

> Replace hand-built estimate/reconcile markdown tables with one grok-oss tool call that prints estimate vs actual wall time and nested tokens, per job and rolled up, from L2-B counters and SQLite.

**North star:** an agent that owes a status table calls `token_report` and pastes the tool result. The Operator never sees a hand-typed table that copied an estimate, printed `0` for a null, or invented a second cost formula.

**Baseline (what exists today):**
- Cost formula: `model_call_cost_for_sessions` in `crates/codegen/xai-grok-shell/src/token_economy/ledger.rs` (lines 640-689). Job cost is input plus output on complete `main` / `agent_turn` rows. Each `event_ulid` counts once. Session ids are deduped. Reasoning tokens are already inside output. Cached tokens are already inside input. `total_tokens` is not the cost. A null input or output is named by `event_ulid` and left out of the sum. Never a stand-in `0`.
- Row reader: `local_usage_events_for_session` in the same file (lines 743-776).
- Live nested counts: `crates/codegen/xai-grok-pager/src/app/agent_view/l2_token_tracking.rs`. In-memory `AtomicU64` high-water plus current sample. Subagents list paint reads this map. It is session usage, not included SuperGrok period limits, not SuperGrok dollar credits, and not console team prepaid / console API credits.
- TOON: `xai_grok_tools::util::toon` and `GROK_TOOL_RESULT_FORMAT`. L2-D owns making TOON the default. This tool emits TOON as one of two formats.
- Prompts: project `AGENTS.md` and bundled `what` / `subagent` skills now tell agents to call `token_report`. Host `~/.grok/AGENTS.md` § *Comprehensive estimate and reconcile* still tells agents to build the markdown table by hand (outside this write set).
- L2-B published API: `/home/hunter/.agents/reports/l2-b-published-api.md` EXISTS (contract note). `plans/l2-b-published-api.md` MISSING. `plans/l2-b-live-token-accounting.md` MISSING. B's note says product code is not implemented this turn. Proposed live module is `xai_grok_shell::token_economy::live`. L2-C must call `model_call_cost_for_sessions`, `live_slot`, `live_rollup`, `live_cost`, and `finish_job_cost` after those land. Do not copy ledger math. Do not guess AtomicU64 slot indices. Slot order B published: Input 0, Output 1, Cached 2, Reasoning 3.
- Gap: no live `token_report` tool yet. Golden fixtures and failing exact-match tests exist. Renderer returns empty on purpose (TDD red). Live counters wait on B's implement.

---

## Goals

### G1. One tool, two formats
`token_report` is a read-only grok-oss harness tool. Input selects format: human-readable markdown table, or TOON. Agents call the tool. They do not hand-write these tables again.

### G2. Operator contract columns
Human table columns, in this order:

| job | estimate wall | estimate nested tokens | actual wall or elapsed | actual nested tokens | under / over |

Per job, then one roll-up row. Spell every number. Never write `same` for a copied figure. Cell values for the last column are `under`, `over`, or `still open`.

### G3. Numbers from B and the ledger, not a second formula
- Finished actual nested tokens: `model_call_cost_for_sessions` over that job's L2 session and every nested L3 session, once each (input plus output). GitHub #224: all tokens in an L3 contribute to the L2 cost.
- Live actual nested tokens: L2-B counters in `l2_token_tracking.rs`. A running cell may use a trailing `+`. That figure is context growth, not a second cost.
- Wall: job wall is spawn to the finish report. A one-second check is not the job wall. Live rows print elapsed.
- Null fields: omit the figure, or say the output total was not stored. Do not print `0`. Do not copy the estimate. Do not write `not_fetched` for this cell. Leave the Billing Credits card wire `not_fetched` alone. That is a different meter.

### G4. Format rules (wall and nested tokens)
Nested tokens: **N.Nk** if under **1000.0k**; **N.Nm** if **1000.0k** or more (example: **20.4m**, not **20410.0k**). Always one decimal on k and m.
Wall: **minutes** if at most **60**; **N.N hours** if over **60** minutes and at most **24** hours; **N.N days** if over **24** hours. Never leave a figure over **60** minutes written as minutes.
`under` when the job has finished and both actual wall and actual nested tokens are at or below estimate. `over` when the job has finished and actual wall or actual nested tokens is above estimate. `still open` when the job has not finished. Roll-up is `still open` when any job is still open.

---

## Workstreams
| # | Work | Done when |
|---|---|---|
| W0 | This plan + golden human table + golden TOON + failing exact-match tests | files on disk; tests red until the renderer exists |
| W1 | Prompt update: project `AGENTS.md` and bundled `what` / `subagent` skills tell agents to call `token_report` | a later reader cannot find those files telling agents to hand-build the table |
| W2 | Pure formatter: `render_token_report` matches both goldens | exact-match tests green; still no live counters |
| W3 | Live tool after L2-B publishes: register `token_report`, read B's counters and SQLite, reuse `model_call_cost_for_sessions` | tool call prints the same columns from live data |
| W4 | Host `~/.grok/AGENTS.md` comprehensive-estimate section (outside this write set) | host pin says call the tool; tracked as leftover |

Tests in Rust (`xai-grok-tools` unit tests against fixture files). No Python drivers. No throwaway bash as the product. Horizon must not run cargo.

## Success criteria
1. An agent that needs estimate vs actual calls `token_report` and does not hand-write the six-column table.
2. Human-table golden and TOON golden exist and a named test asserts exact output for each.
3. Cost is input plus output from `model_call_cost_for_sessions`. No forked ledger math.
4. Live nested counts come from L2-B. This plan waits on B's published API rather than inventing it.

## Non-goals
Dock / subagent visibility (L2-A). AtomicU64 counters and SQLite schema (L2-B). Inventory JSON-vs-TOON harness defaults (L2-D). `viewer.rs`. Making TOON the default tool-result format (L2-D). A second cost formula. Hand-building the report table in the product as a stand-in for the tool.
