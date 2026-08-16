# grok-oss SQLite extras vs 1.0.3 (2026-08-13)

Diagnosis only. No product edits. SuperGrok is paid. This report says **included SuperGrok period limits**, not "free SuperGrok." FORK and spend copy still say "free SuperGrok period." That is language residual, not a SQL table.

Compared: this onto tree (`onto-xai/b13fa526f511`), GitHub `SurmountSystems/grok-oss` `main` `grok_oss/mod.rs` (same schema v1), and xAI Grok Build 1.0.3 session-search schema (`e5fd4816` `fts.rs`, same as this tree). 1.0.3 has no `grok_oss` module (raw path 404). GitHub `main` `dispatch_show_spend` still writes the ledger. This tree stubs `/spend`.

## 1. What the grok-oss SQL database is

| Item | Fact |
|------|------|
| Path | `$GROK_HOME/grok_oss.db` (`xai_grok_config::grok_home()`, default `~/.grok/grok_oss.db`). Override: `[token_economy] grok_oss_database_path`. |
| Crate | `xai-grok-shell` module `grok_oss` (`crates/codegen/xai-grok-shell/src/grok_oss/mod.rs`). Ledger helpers: `token_economy/ledger.rs`. |
| Engine | `rusqlite` 0.37 bundled + `xai_sqlite_journal` (NFS-safe journal, busy timeout). Not sqlx. |
| When it opens | Fail-open. **This tree:** `try_open_from_token_economy_config` from `/limits` double-entry section (`limits_snapshot.rs`) and from `build_double_entry_report` (called by that section). **Does not** open at process start. **`/spend` does not open it** (see below). First successful open creates parent dirs, applies journal mode, runs migrate. |
| What it is not | Not the session store. Sessions stay `$GROK_HOME/sessions/<enc-cwd>/<id>/` (jsonl + json). Comment in `grok_oss/mod.rs`: "Not an upstream session database." |

Schema version is only **1**. `migrate` creates `meta`, then if `schema_version` < 1 applies `SCHEMA_V1` and stamps `1`. There is no v2 in this tree or on GitHub `main`.

**SCHEMA_V1 tables**

- `meta(key, value)`
- `local_usage_event` (PK `event_ulid`; indexes on `session_id`, `timestamp_utc`)
- `remote_meter_sample` (index on `source, sampled_at`)
- `reconciliation_run`

Column `local_usage_event.sampling_identity` exists. Ingest from `usage.jsonl` always sets it to `None` (`From<UsageJsonlRow>`).

## 2. Every SQLite open path this product uses

| Database | Path | Role | 1.0.3? | Surmount extra? |
|----------|------|------|--------|-----------------|
| grok-oss durable store | `$GROK_HOME/grok_oss.db` | Token Economy ledger | No (no module) | Yes. Schema present. Writes dropped. |
| Session search FTS | `$GROK_HOME/sessions/session_search.sqlite` | Rebuildable FTS5 cache (`session_docs` + `session_docs_fts` + `meta`) | Yes. Schema version 4, same `CREATE TABLE` as 1.0.3 `fts.rs` | No extra tables found. |
| Memory index | workspace `index.sqlite` | Memory chunks + FTS + optional vec | Upstream memory crate | No Surmount extra tables found. |
| Fast worktree | worktree crate `meta` + `worktrees` | Worktree inventory | Upstream | No Surmount extra tables found. |
| Codex / Cursor | Foreign `state_*.sqlite`, Cursor `store.db` / `state.vscdb` | `session_reader` + `xai-grok-foreign-sessions` **read** other products | Readers are Surmount (FORK A4). Those files are not grok-oss. | Not our schema. |
| Journal helper | `xai-sqlite-journal` | Open/journal only | Used by 1.0.3 search/memory/worktree | Not a feature database. |

No sqlx product store. The only `sqlx` hit is example text in `turn_summary.rs`.

## 3. Extra feature table

Status meanings: **present** = this tree still implements the named contract; **dropped** = FORK / Surmount `main` had a working SQL-backed surface and this tree does not; **schema-only unused** = table or column exists, no product writer in this tree; **not SQL** = FORK or operator examples that live on disk as json/jsonl, not in `grok_oss.db`; **unproven** = not shown by code or git.

| Extra feature (plain English) | Table / column / migration | FORK / docs claim | Status now | Evidence |
|-------------------------------|----------------------------|-------------------|------------|----------|
| Separate grok-oss SQLite file | `grok_oss.db`, `SCHEMA_VERSION = 1`, `open_at` / `try_open_*` | FORK Token Economy pillar 4: durable store `$GROK_HOME/grok_oss.db` (fail-open, multiproc busy timeout, no secrets, additive schema) | **present** (file + migrate) | Module and tests in `grok_oss/mod.rs`. GitHub `main` same file. |
| Local spend book (ingest session `usage.jsonl` into SQL) | `local_usage_event` + `ingest_all_sessions_usage` | FORK: double-entry local `usage.jsonl` book vs Management on `/spend` and a `/limits` section | **schema-only unused** in this tree | Ingest exists in `ledger.rs`. Only caller is `build_double_entry_report_with_options(..., refresh=true)`. This tree never passes `true`. |
| Persist Management / prepaid / postpaid samples | `remote_meter_sample` + `insert_remote_meter_sample` | Same FORK pillar 3/4 | **schema-only unused** | Product `try_insert_remote_meter_sample` is only on GitHub `main` `/spend`. This tree: function used in ledger unit tests only. |
| Persist a reconciliation history row | `reconciliation_run` + `insert_reconciliation_run` | FORK `/spend` double-entry | **schema-only unused** | Same: only GitHub `main` `/spend` inserts. |
| Sampling identity on each local event | `local_usage_event.sampling_identity` | Plan sketch: `'supergrok_session' \| 'console_key' \| null` (`.agents/plans/token-economy-options-2026-08-03.md`) | **schema-only unused** | Ingest sets `None`. No writer fills the column. |
| `/spend` shows real local vs remote books and refreshes the DB | Uses all three tables | FORK checked: `/spend` + aliases `/double-entry` / `/ledger` | **dropped** | This tree `dispatch_show_spend` formats `DoubleEntryReport::default()` only (`status.rs`). Slash still emits `Action::ShowSpend`. GitHub `main` still ingests `usage.jsonl`, inserts remote samples, inserts `reconciliation_run`, then formats the real report. |
| `/limits` spend section reads the local book | `summarize_local_book` + `latest_remote_sample(..., "management_usage_series")` | FORK: `/limits` section with gap honesty | **present**, but ingest never runs so the book stays empty unless an older binary already filled the file | `format_limits_double_entry_section` in `limits_snapshot.rs`. `refresh_from_sessions` is false. |
| Per-session `usage.jsonl` companion | Not SQL. Session dir `usage.jsonl` | FORK: usage.jsonl main + subagent turns | **present** (jsonl, not SQL) | `usage_log.rs` + `record_response_token_usage` in `sampler_turn.rs`. Schema comment: "SQL-friendly" for later ingest. |
| Last-session auto-open on plain `grok-oss` start | Not SQL. `list_summaries` over session json | Operator named it. FORK does not pin a SQL last-session pointer. | **present** in this tree via session files, **not** `grok_oss.db` | `MaterializeCtx.open_last_session_on_start` + `try_most_recent_session_id`. No last-session table. |
| Continue interrupted turn | Not SQL. `canceled_turn_resume.json` | FORK: session marker, reopen re-queues | **write present; load apply not in `handle_session_loaded`** | Writer: `session/canceled_turn_resume.rs`. `handle_session_loaded` (`load.rs`) never calls `load_canceled_turn_resume`. Test `session_loaded_applies_cancel_resume_marker_and_toasts` still expects toast + SendPrompt. Observed, not finished. |
| Billing identity / included-period poll / exhaust memo | Not SQL | FORK: `$GROK_HOME/exhausted_credits/` + included poll | **present as json files** | `exhausted_credits/{fingerprint}.json`; `included_poll_history/{identity}.json`. |
| Session titles | Session json `generated_title` + search `session_docs.title` | FORK window titles / generated titles | **present**; search title column is **1.0.3**, not a Surmount extra table | Persistence tests + `session_search` `session_docs.title`. |
| Todo board persistence | Not SQL. `resources_state.json` / `plan.json` | FORK: todo board + plan.json honesty | **json present**; AutoCompact UI wipe is a paint bug (other report), not a missing SQL table | Tools `persistence.rs`. |
| Session notes `/note` | Not SQL | FORK: session-local store | **in-memory only** | `dispatch_add_session_note` does not write a file or DB. |
| Extra schema v2+ tables (last-session row, cancel-resume row, titles table, todo table, billing identity table) | Would be `SCHEMA_VERSION >= 2` | Comment: "later Surmount-only durable state can add tables via additive migrations" | **unproven / not in any tree walked** | This tree and GitHub `main` both stop at v1. No `CREATE TABLE` beyond the four Token Economy tables. |

## 4. What 1.0.3 still has vs what Surmount added

**1.0.3 still has (same in this tree)**

- Session trees: directories + jsonl, not a sessions SQLite.
- `session_search.sqlite` FTS5 (`meta`, `session_docs`, `session_docs_fts`, schema 4).
- Memory `index.sqlite` (`meta`, `chunks`, `chunks_fts`, optional `chunks_vec`).
- Worktree SQLite (`meta`, `worktrees`).
- `xai-sqlite-journal`.

**Surmount added (not in 1.0.3)**

- `$GROK_HOME/grok_oss.db` and the Token Economy schema family.
- `usage.jsonl` next to the session (companion for ingest).
- `/spend` + `/limits` spend section that were supposed to fill and read that DB.
- Foreign Codex/Cursor SQLite **readers** (not extra grok-oss tables).

**Surmount `main` vs this restack (SQL-backed)**

- Schema file: **same** (v1, four tables). Restack did not drop extra tables from `grok_oss/mod.rs` because those extra tables were never in `main`.
- Product wire: **dropped**. GitHub `main` `dispatch_show_spend` fills `local_usage_event`, `remote_meter_sample`, and `reconciliation_run`. This tree prints an empty default report and never touches the file from `/spend`.
- `/limits` still opens the DB and summarizes. Without `/spend` ingest, new `usage.jsonl` lines never become SQL rows. Old rows from a pre-restack binary would still show if the same `$GROK_HOME/grok_oss.db` file is on disk.

## 5. If the extra migration never ran

There is only migration v1. Runtime:

| Situation | What happens |
|-----------|----------------|
| File missing | First `try_open` creates it and applies v1. Fail-open: open error logs at debug and returns `None`. `/limits` spend section then uses an empty in-memory book (no crash). `/spend` in this tree never opens the file. |
| File exists, `schema_version` 0 or missing | `migrate` applies `SCHEMA_V1` (IF NOT EXISTS) and stamps 1. |
| File exists, `schema_version` 1 | No further SQL. Hypothetical later tables are not created. None exist in code. |
| File exists with extra tables from an unpublished local build | This binary leaves them alone. It never reads them. Unproven whether the operator's live file has such tables (this walk did not open `~/.grok/grok_oss.db`). |
| Insert/summarize without migrate | Would error. Every open path calls `migrate` first. Insert helpers fail-open (debug log, no crash). |
| `/spend` stub | Silent empty report. No crash. Looks like "the extras are gone" even when the schema file is fine. |

## 6. Observed continue-interrupted-turn / last-session (not SQL)

Last-session-on-start is implemented in `session_startup.rs` from session summary files, not SQLite.

`handle_session_loaded` has no `load_canceled_turn_resume` call. Names `session_looks_interrupted_mid_work` / `try_auto_resume_error_idle_on_reopen` are **not** in this tree. A cancel-resume apply test still exists. Do not treat that as a missing SQL table.

## 7. Leftovers not walked

- Live `$GROK_HOME/grok_oss.db` row counts (no sqlite inspect this turn).
- Local `git log -S` / `git show` of older Surmount commits (no shell). Public `main` `grok_oss/mod.rs` is enough to show schema did not grow past v1.
- Memory / worktree schemas byte-for-byte vs 1.0.3 (CREATE TABLE names match this tree; no Surmount extra tables found).
- Whether catalog tests for `/spend` ingest still exist and fail.
- Config.toml Token Economy knobs (other agent).

## 8. Counts

- **Present extras:** 2 (grok_oss.db module + migrate; `/limits` spend section still opens and reads the book).
- **Dropped extras:** 1 (`/spend` real ingest, remote-sample write, and `reconciliation_run` persist vs Surmount `main` / FORK).
- **Unused schema:** 4 (`local_usage_event` with no ingest caller; `remote_meter_sample` with no product insert; `reconciliation_run` with no product insert; `sampling_identity` never filled).
