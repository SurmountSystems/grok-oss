# Token Economy options (proposed product direction)

**Date:** 2026-08-03
**Status:** proposed plan only. Not accepted until the operator approves via plan CTAs or explicit freeform approve.
**Scope:** additive product options on top of shipped limits-first / dual-auth meters. Do not invent C4 SuperGrok included debit or fold meters together.

---

## 1. Problem (plain language)

Operators already have strong **limits-first** and **meter honesty** product (free SuperGrok period allowance, SuperGrok top-up dollars, console team prepaid, postpaid class, team default credits). Pain that remains is **economy of use**, not missing wallet labels:

1. **Effort burn** — Auto-run `/implement` can re-queue high `--effort` (4–5 reviewers). Economic mode today only soft-caps context at 200k; it does **not** cap implement-loop effort. High effort multiplies subagent turns and spend while the free SuperGrok period allowance still has headroom (or while console prepaid is live).
2. **Spend opacity** — Local turns write `usage.jsonl` (tokens and optional cost ticks). Remote Management can show team usage series and prepaid/postpaid. There is **no** double-entry view that says “local calculated spend vs remote reported spend” for the same window.
3. **Pacing blindness** — The GRLD-inspired TUI credit chrome shows **used percent** of the free SuperGrok period (and related honesty). It does **not** show whether you are **ahead or behind** a linear burn for the current period (e.g. “behind by 12%” meaning you have used less than expected at this point in the period, or the inverse).
4. **Upstream session store safety** — Anything that needs durable economy history should **not** migrate or rewrite the main session tree under `$GROK_HOME/sessions/` (jsonl-first, upstream-compatible). Prefer a **separate** SQLite database under `$GROK_HOME`.

Related dogfood pain (already fixed or residual elsewhere, not re-scoped here): period-reset sticky console memo (join `bug-period-reset-flipped-to-console-2026-08-03.md`); C4 free SuperGrok period debit still server-side FAIL (do not invent).

---

## 2. What already exists

| Topic | What exists | Paths / symbols |
|-------|-------------|-----------------|
| **Economic mode** | Soft-cap effective context at 200k (Grok 4.5 price cliff). Default **on**. `/economic-mode` + `[ui] economic_mode`. Settings live toggle. | `xai-grok-shell` `util/config/economic_mode.rs` (`ECONOMIC_CONTEXT_CAP`, `apply_economic_context_cap`); pager `appearance/cache` + slash `economic_mode.rs`; user-guide `05-configuration.md` |
| **Economic × implement effort** | **No product clamp today.** `clamp_implement_effort_for_economic_mode` is identity; tests require explicit `--effort N` be honored. Stale module comment still claims clamp-to-1. FORK: “economic mode does not rewrite it.” | `xai-grok-pager` `app/auto_implement.rs`; FORK auto-run bullet |
| **Auto-run implement** | After successful turn, enqueue follow-up `/implement` block; default on `[ui] auto_run_implement` | `auto_implement.rs`; `ui_config.auto_run_implement` |
| **Implement-loop effort (1–5)** | Host skill: reviewer fan-out (1→1 slot … 5→6 slots). Not reasoning effort. | `~/.agents/skills/implement/SKILL.md` |
| **Reasoning effort** | Separate: `/effort`, `--effort` on models (`low`/`high`/…). | `slash/commands/effort*.rs`; shell sampling types |
| **Limits-first / dual-auth** | `auto_use_included_limits` default true for new installs; SuperGrok-first ranking; extras-before-console; period-reset memo clear (shipped join). | residual §4; FORK dual-auth + billing bullets |
| **Credit bar / status** | Compact status `XX%` = free SuperGrok period used %; colors at 80/100; click → Limits. `/usage` + `/limits` detail. **No ahead/behind pacing.** | `views/credit_bar.rs` (`credit_bar_line*`, `CreditBalance`, `period_end_*`, `period_type`) |
| **Period semantics** | Wire `currentPeriod` / `USAGE_PERIOD_TYPE_WEEKLY` (or monthly) + `period_end`; product labels “Weekly limit” / “Monthly limit”. | `extensions/billing.rs` `included_usage_and_period_end`; credit_bar `usage_label` |
| **Management remote spend** | Keyring URL `https://management-api.x.ai`; prepaid GET; postpaid preview; **POST usage series** (7-day default window); team default credits own line. | `auth/xai_management.rs`; Item 5 join `impl-item5-spend-series-default-credits-2026-08-03.md` |
| **Local turn spend** | Append-only per-session `usage.jsonl` (tokens, optional `cost_usd_ticks`, main vs subagent). Fail-open. Schema “SQL-ready”; no product SQL ingest. | `session/usage_log.rs`; research `doc/dev/research/usage-jsonl-2026-07-25.md` |
| **Poll history** | Durable SuperGrok included poll ring under `$GROK_HOME/included_poll_history/` (files, not SQLite). Process ring + flat-poll honesty. | residual §4; join durable-included-poll-history |
| **Shared rate limits** | Multiproc flock JSON under `$GROK_HOME/rate_limits/` (billing + Management already observe). Not an economy ledger. | crate `grok-rate-limit` |
| **Session store** | Filesystem: `$GROK_HOME/sessions/<encoded-cwd>/<session-id>/` (jsonl, plan.md, usage.jsonl, …). **Not** a single product SQLite session DB. | user-guide `17-sessions.md` |
| **Other SQLite** | `worktrees.db`, memory `index.sqlite`, external Codex/Cursor readers. Journal helper `xai-sqlite-journal`. | `xai-fast-worktree`, memory backend |
| **Token economy as product name** | No shipped “token economy mode” feature beyond economic mode + limits stack. Residual §2h “structured conversations for token efficiency” is process/UX plan, parked. | residual §2h |

**Docs vs code gap:** `economic_mode.rs` crate docs still say auto-queued implement loops clamp `--effort` to 1. Code and FORK disagree (no rewrite). Any new effort policy should fix that comment and re-document intentionally.

---

## 3. Proposed product surface (not accepted until approved)

### 3a. Config knobs (readable names)

Prefer `[ui]` or a small `[token_economy]` table only if several knobs need grouping. Proposed names (wire names in parens; plain thought first):

| Plain name | Proposed key | Default (proposed) | Role |
|------------|--------------|--------------------|------|
| Economic mode (existing) | `[ui] economic_mode` | true | Context soft-cap 200k |
| Cap implement-loop effort while economic mode is on | `[ui] economic_mode_caps_implement_effort` or always-on when economic | true when economic on | Master switch for §3b |
| Maximum implement-loop effort under economic mode | `[ui] economic_mode_max_implement_effort` | **3** | Hard ceiling (1–5) |
| Desired implement-loop effort under economic mode | `[ui] economic_mode_desired_implement_effort` | **1** or **2** (open Q) | Default when auto-queue has no `--effort`, or when filling missing flag |
| Show period pacing in TUI | `[ui] show_period_pacing` | true (or on when economic) | Ahead/behind chrome |
| Local spend ledger enabled | `[token_economy] local_spend_ledger` | true | Write/aggregate into separate SQLite |
| Reconcile remote Management usage | `[token_economy] reconcile_management_usage` | true when management key present | Double-entry pull |

Do **not** use plan-step codes in keys or filenames.

### 3b. Economic mode: implement-loop effort policy

**Scope of “effort” here:** the implement skill’s integer **1–5** (reviewer fan-out), **not** model reasoning effort (`/effort high`).

**Proposed policy when economic mode is on:**

1. **Hard ceiling 3** — never run implement-loop effort above `economic_mode_max_implement_effort` (default 3). Applies to:
   - Auto-run enqueue path (`clamp_implement_effort_for_economic_mode` becomes real again, with ceiling 3 not 1).
   - Optional: explicit user `/implement --effort 5` while economic mode is on → clamp to 3 **and toast** (“economic mode: implement effort capped at 3”). Open question whether explicit operator override should win (see open Qs).
2. **Configurable desired effort** — when auto-queue extracts a block **without** `--effort`, inject `--effort <desired>` (config). When block has effort **above** max, clamp to max. When block has effort **between** desired and max, leave as written (or optionally clamp down to desired; default: leave if ≤ max).
3. **When economic mode is off** — no product clamp (today’s honor-explicit behavior).
4. **Does not apply by default to** bare main-thread agent work or non-implement subagent spawns, unless a later slice adds a global “max parallel reviewers” policy. First ship: implement auto-run + slash `/implement` entry path only.

**Skill dual-pin:** host `implement` skill should document that product may rewrite `--effort` under economic mode; product is source of truth for the clamp.

**Tests (TDD):** red contracts for auto-queue with `--effort 5` → enqueued as 3 when economic on; `--effort 2` stays 2; economic off leaves 5; config desired inject when missing; toast text.

### 3c. TUI pacing (GRLD-inspired)

**Inspiration:** GRLD-style pacing chrome (ahead / behind for the period). **Not present in this tree** under the name GRLD; build on existing credit bar / limits surfaces.

**What “week” means (proposed default):** the **free SuperGrok billing period** from session billing (`currentPeriod` / `period_type` weekly or monthly + `period_end_at`), **not** a fixed Monday–Sunday calendar week unless wire period is weekly and starts on that boundary. Label with the same Weekly/Monthly words already used on `/usage`.

**Meter for pacing (proposed default):** free SuperGrok period **used percent** (`CreditBalance.usage_pct`) vs **expected linear burn** from period start → now → period end.

```
expected_pct = 100 * (now - period_start) / (period_end - period_start)
delta = usage_pct - expected_pct
# delta > 0 → ahead of linear burn (used more than time share)
# delta < 0 → behind linear burn (used less; more room left than time share)
```

Display copy (proposed):

- Footer / credit bar secondary chip: `+12% ahead` / `−8% behind` (or `12% ahead of pace`) next to or under `XX%`.
- `/limits` and `/usage`: one plain line, e.g. “Free SuperGrok period pacing: 12% ahead of linear burn (period ends …).”
- **Honesty:** if `period_start` or `period_end` missing, show no pacing invent (gap or omit).
- **Console-live:** do **not** sell SuperGrok pacing as console spend; either hide SuperGrok pacing while console is live principal, or label “SuperGrok period (not live principal).” Prefer hide-or-label per existing meter honesty.

**Where chrome lives:** extend `credit_bar.rs` compact line and/or status hover; full sentence on limits snapshot / usage summary. No second fake meter that looks like prepaid $.

**Not first-class:** pacing for console team prepaid $ or postpaid class (optional later; needs different “budget” definition).

### 3d. Double-entry spend tracking

**Goal:** bookkeeping-style **two books**:

| Book | Source | Content |
|------|--------|---------|
| **Local ledger** | Aggregated from session `usage.jsonl` (+ optional live fold on turn end) | Tokens, `cost_usd_ticks` when present, principal/host class if known, session/work ids, timestamps |
| **Remote book** | Management API (existing clients) | Team usage series class totals (OAuth vs API), prepaid balance snapshots, postpaid preview; optional SuperGrok billing samples already in poll history |

**Reconciliation (proposed):**

- Window: configurable (default align to usage series 7-day window **or** free SuperGrok period; open Q which).
- Rows: local sum of `cost_usd_ticks` (where present) vs remote series USD for **API class** when console path is used; SuperGrok path may only have included % + extras $ (not full token $).
- UI: `/limits` or new `/spend` slash: “Local calculated (known cost rows): $X · Remote Management series (API class): $Y · Gap: …” with honesty when local cost_missing high.
- **Do not** invent dollars when cost ticks are missing; show token counts and “cost not reported on N calls.”
- Management key path: reuse existing resolve (`[endpoints] management_api_key` / keyring / env). No new secret store.

**Meters stay distinct** in copy: free SuperGrok period % ≠ SuperGrok top-up $ ≠ console team prepaid ≠ postpaid OAuth/API class ≠ team default credits.

### 3e. Separate SQLite database

**Proposed path:** `$GROK_HOME/token_economy.db` (or `spend_ledger.db`). Use `xai-sqlite-journal` for NFS-safe journal mode (same pattern as worktrees).

**Why not main session store:** sessions are upstream-shaped **directories + jsonl**. Mixing ledger tables into a hypothetical sessions.sqlite (or rewriting usage.jsonl into sessions DB) risks recon/upstream merge pain and breaks external tools that read jsonl. Separate DB is additive and fail-open.

**Minimal schema sketch (proposed):**

```sql
-- meta
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

-- one row per ingested usage.jsonl event (idempotent on event_ulid)
CREATE TABLE local_usage_event (
  event_ulid TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  work_ulid TEXT,
  timestamp_utc TEXT NOT NULL,
  turn_type TEXT NOT NULL,
  agent_kind TEXT NOT NULL,
  model_id TEXT,
  input_tokens INTEGER,
  output_tokens INTEGER,
  cached_tokens INTEGER,
  reasoning_tokens INTEGER,
  total_tokens INTEGER,
  cost_usd_ticks INTEGER,  -- null if cost_missing
  cost_missing INTEGER NOT NULL,
  incomplete INTEGER NOT NULL,
  sampling_identity TEXT,  -- 'supergrok_session' | 'console_key' | null
  ingested_at TEXT NOT NULL
);

-- remote snapshots (Management / billing pulls)
CREATE TABLE remote_meter_sample (
  id INTEGER PRIMARY KEY,
  source TEXT NOT NULL,  -- 'management_usage_series' | 'prepaid' | 'postpaid' | 'supergrok_included'
  sampled_at TEXT NOT NULL,
  window_start TEXT,
  window_end TEXT,
  payload_json TEXT NOT NULL  -- hermetic-friendly structured fields, no secrets
);

-- optional reconciliation runs
CREATE TABLE reconciliation_run (
  id INTEGER PRIMARY KEY,
  ran_at TEXT NOT NULL,
  window_start TEXT NOT NULL,
  window_end TEXT NOT NULL,
  local_cost_usd_ticks INTEGER,
  remote_api_class_usd_cents INTEGER,
  remote_oauth_class_usd_cents INTEGER,
  notes TEXT
);
```

**Ingest:** background or on `/limits` / session end: scan recent session `usage.jsonl` → upsert by `event_ulid`. Fail-open if DB locked.

**Do not** put secrets (management key, JWTs) in this DB.

---

## 4. Risks and open questions

### Risks

- **Effort clamp vs operator intent:** FORK currently promises explicit `--effort` is honored. Changing that needs clear toast + config + doc update; some operators will want economic context cap without effort cap.
- **Pacing misread:** “ahead” can sound good (ahead of schedule) but means **burning faster** than linear. Copy must say “ahead of linear burn” / “using faster than pace.”
- **Local vs remote apples-to-oranges:** SuperGrok included is **percent**, not USD; console series is **team** USD; local cost ticks may be incomplete. Reconciliation honesty is mandatory or the feature lies.
- **SQLite multiproc:** multiple grok-oss processes; need flock/busy timeout; do not block the sampler turn on ledger write.
- **Upstream recon:** keep all new files outside `FORK_PATHS` session semantics; new crate or module under Surmount-owned paths if needed (`grok-*` name).

### Open questions (high-signal only)

**Q1.** When economic mode is on and the user types `/implement --effort 5` **explicitly**, should product (A) clamp to max 3 + toast, or (B) honor 5 and only clamp **auto-run** queues?

**Q2.** Default **desired** implement effort under economic mode: 1 (cheapest) or 2 (light multi-review)?

**Q3.** Pacing “week”: confirm free SuperGrok **billing period** (recommended) vs strict calendar week.

**Q4.** Double-entry primary window: free SuperGrok period, last 7 calendar days (series default), or operator-chosen range on `/spend`?

**Q5.** Ship order: effort cap first (cheapest, high leverage) before pacing and ledger, or all as one “token economy” settings section?

---

## 5. Suggested ship slices

| Order | Slice | Agent-doable? | Notes |
|-------|-------|---------------|-------|
| 1 | **Economic implement-effort ceiling + desired default** | Yes | Restore real clamp in `auto_implement.rs`; config keys; settings row; fix stale economic_mode docs; TDD; user-guide + FORK honesty |
| 2 | **Period pacing chrome** | Yes | Pure math from `period_start`/`period_end`/`usage_pct`; credit_bar + limits/usage lines; honesty when dates missing; console-live label rules |
| 3 | **Separate SQLite + local ingest** | Yes | Open/create `$GROK_HOME/token_economy.db`; ingest `usage.jsonl` by event_ulid; no UI required for v1 |
| 4 | **Remote sample persist + reconcile report** | Yes | On explicit limits/spend collect: store Management series snapshot; print local vs remote gap with honesty |
| 5 | **`/spend` or Limits panel section** | Yes | Operator-facing double-entry view; optional sparkline later |
| 6 | **Dogfood + residual re-rank** | Operator-gated | Live management key + dual-auth; rebuild binary |

**Operator-gated:** management key + team id already required for remote book; free SuperGrok OAuth for pacing; no C4 invent.

---

## 6. Out of scope by default

- Inventing free SuperGrok period **debit** (C4 server ticket remains human/xAI).
- Scraping console.x.ai HTML.
- Folding team default credits into prepaid $N.
- Replacing session jsonl with SQLite.
- Cap model **reasoning** effort under economic mode (unless operator later asks).
- Full Business Usage chart UI (Item 5 series skeleton is enough until dogfood asks).
- C4 multi-poll debit proof; Phase R rate limits by API type (separate residual).
- Structured conversation UX residual §2h (related token efficiency, different plan).
- Second SuperGrok OAuth as a pacing target (optional later).

---

## 7. Acceptance sketch (if approved)

1. With economic mode on and default config, auto-run never enqueues implement effort above 3; desired effort applies when missing.
2. Credit / limits surface shows linear-burn pacing when period bounds known; never invents when unknown.
3. Local ledger DB exists under `$GROK_HOME` separate from sessions; ingest does not break turns (fail-open).
4. With management key, operator can see local known-cost sum vs remote series for a window with explicit gap honesty.
5. Meters remain named distinctly in all new copy.
6. Upstream session layout unchanged.

---

## 8. Critical implementation map (for implementers after approve)

| Area | Touch |
|------|--------|
| Effort clamp | `crates/codegen/xai-grok-pager/src/app/auto_implement.rs` |
| Economic config | `economic_mode.rs`, `ui_config.rs`, settings modal, user-guide `05-configuration` |
| Pacing | `views/credit_bar.rs`, `limits_snapshot.rs`, billing period fields |
| Management remote | `auth/xai_management.rs` (reuse POST usage) |
| Local usage | `session/usage_log.rs` (read path; keep write path) |
| New DB | new module under shell or small `grok-token-economy` crate; path under GROK_HOME |
| Docs | FORK short bullet when shipped; residual §4 only if economy becomes open residual track |

---

*End of plan. Wait for plan panel CTA or freeform approve before any product implementation.*
