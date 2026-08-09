# Impl report: address C4 as hard as client law allows (2026-08-07)

**Branch:** `fixes-2` (shared workspace)
**Mandate:** exhaust client levers + ship complete paste-ready xAI ticket; residual honesty as operator deliverable, not agent shrug
**Banned:** invent free SuperGrok period debit % in chrome, rank, tests, or poll invent

---

## What shipped

### Wave 1 — paste-ready ticket package

| Artifact | Path |
|----------|------|
| **Paste-ready ticket** (title + body + both periods + env + Q1–Q6 + repro + attachments) | [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](c4-xai-ticket-paste-ready-2026-08-07.md) |
| Multipoll A (limits --json) | [`.agents/reports/c4-limits-poll-a-2026-08-08T055844Z.json`](c4-limits-poll-a-2026-08-08T055844Z.json) |
| Multipoll B (~35s later) | [`.agents/reports/c4-limits-poll-b-2026-08-08T055844Z.json`](c4-limits-poll-b-2026-08-08T055844Z.json) |
| Durable history tail summary | [`.agents/reports/c4-poll-history-business-tail-2026-08-08.json`](c4-poll-history-business-tail-2026-08-08.json) |

Merged sources: 2026-08-02 evidence package, 2026-08-07 addendum, how-to-fix-c4, live refresh.

### Wave 2 — client lever audit + real product fix

| Lever | Status |
|-------|--------|
| Free-period-first path under dual-auth (Design A) | **Exhausted / correct.** Live `activeDriver=supergrok_free_period`, `console.isLive=false`, both principals `live_poll` OK at 6%. No accidental console primary under headroom. |
| Flat-poll / dual-bill honesty notes | **Shipping and tested.** C6 note present on live human + JSON. Dense multipoll was **not** lighting flat-poll under high-frequency samples (bug). |
| Poll history durability | **Shipped** (`$GROK_HOME/included_poll_history/`). Ring showed 6.0-only series for ticket evidence. |
| Unused wire field inventing free-period % | **None found.** Do not invent. |
| Doctor / limits human text | **Clear enough.** Notes say included % is poll reading not burn proof; team OAuth $ can move without free-period proof. Doctor dogfood block maps proof surfaces. |
| Hop console to "burn free period" | **Forbidden.** Not done. |

**Real product gap fixed (measurement only, not invent %):**

1. **`included_debit_unproven` window** — previously only the last `min_polls` (2) samples. Under dense multipoll those two points are often ~2s apart, so the ≥30s wall never fired even when free period was flat for minutes. Now selects the most recent suffix that spans `min_window`.
2. **`limits --json` export** — `flatPollUnprovenDebit`, `flatPollObservedBuild`, `flatPollObservedExtras` now on `LimitsCliReport` (ticket multipoll evidence).

Code:

- `crates/codegen/xai-grok-shell/src/auth/included_poll_history.rs`
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`

### Wave 3 — residual / FORK honesty

- `RESIDUAL.md` C4 section + rank row 7 + status blurbs: **operator action required** (file paste-ready ticket), client levers exhausted as of 2026-08-07, not agent-parked.
- `FORK.md` free-period-before-credits bullet: points at paste-ready ticket + measurement fix.

### Wave 4 — this report

You are reading it.

---

## Live snapshot used

| Field | Value |
|-------|--------|
| Binary | `grok-oss 0.2.111 (c87f66a61d94) [stable]` (`~/.cargo/bin/grok-oss`) |
| When | 2026-08-08 ~05:58–05:59 UTC multipoll; human limits same evening |
| liveSampling | `supergrok_session` (business) |
| activeDriver | `supergrok_free_period` |
| Free SuperGrok period | **6.0%** used / **94%** remaining (both principals, shared pool) |
| includedSource | `live_poll` both; `pollSucceeded: true` |
| SuperGrok $ extras | **$100.29** |
| console.isLive | **false** |
| Team postpaid OAuth | **$1013.35 → $1013.77** across ~35s multipoll (free period stayed 6.0%) |
| Team prepaid | **$340** |
| Next reset | August 10, 19:25 |

**No multipoll subcommand** exists. Repro is spaced `limits --json` + durable history.

Note: installed binary does **not** yet include the dense-window / JSON field fix until rebuild/install. Evidence package still valid from multipoll deltas + history ring.

---

## Client levers audit table

| Item | Exhausted? | Needs xAI? |
|------|------------|------------|
| Stay SuperGrok session + proxy under free-period headroom | Yes (path correct) | No |
| Dual-auth no false console primary under headroom | Yes | No |
| Compact free-period % honesty | Yes | No |
| C6 / dual-bill notes | Yes | No |
| Durable poll history | Yes | No |
| Dense multipoll flat detector + JSON export | **Fixed in-tree this wave** | No (measurement) |
| Force free period % client-side | **Banned** | — |
| Mash team $ into free-period chrome | **Banned** | — |
| Free period / Build % actually debit with session traffic | **Cannot** | **Yes (C4 pass)** |

---

## TDD red → green

| Test | Contract | Result |
|------|----------|--------|
| `dense_high_frequency_flat_series_marks_unproven_when_wall_spans_min_window` | Dense 2s-interval flat free-period series spanning ≥30s must mark unproven | green |
| `dense_flat_then_step_in_recent_window_clears_unproven` | Step inside recent window clears | green |
| Existing `included_poll_history` suite (15 tests) | Prior flat/step/durable contracts | green |
| `limits_snapshot_sets_flat_poll_from_history_not_only_tests` | JSON exports `flatPollUnprovenDebit` + observed flags | green |
| `human_and_json_surface_flat_poll_note_once_no_dedupe_double` | report fields pass through | green |
| Related `flat_poll` pager filters | green | |

Commands:

```bash
cargo test -p xai-grok-shell --lib included_poll_history
cargo test -p xai-grok-pager --lib -- flat_poll limits_snapshot_sets_flat_poll
cargo fmt -p xai-grok-shell -p xai-grok-pager
cargo clippy -p xai-grok-shell --lib -- -D warnings
cargo clippy -p xai-grok-pager --lib -- -D warnings
```

(`--all-targets` clippy on pager still hits pre-existing unrelated test-only lints; lib clean.)

---

## What operator must still do vs what agents finished

| Owner | Done / owed |
|-------|-------------|
| **Agents finished** | Paste-ready ticket package; multipoll evidence capture; residual/FORK honesty; dense flat-poll measurement fix + JSON export; TDD green |
| **Operator still must** | **Copy title + body from the paste-ready file and file with xAI billing/support once.** Attach multipoll JSON if useful. Track ticket id. Rebuild/install if they want live `flatPollUnprovenDebit` on the installed binary. |
| **xAI still owns** | Free SuperGrok period / Build productUsage debit and settlement under cli-chat-proxy |

**Explicit:** C4 is **not** "fixed by client." C4 is **actionable operator deliverable** with a complete package, not soft-parked for later by agents.
