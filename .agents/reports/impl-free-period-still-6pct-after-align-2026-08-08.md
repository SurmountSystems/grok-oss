# Free SuperGrok period still ~6% after SessionToken align: dogfood forensics + next client fix

**Date:** 2026-08-08
**Branch:** `fixes-2`
**Binary after this pass:** `grok-oss 0.2.111` via `just install` → `~/.cargo/bin/grok-oss`
**Meters kept distinct:** free SuperGrok period used % ≠ SuperGrok dollar credits ≠ console team prepaid ≠ team postpaid OAuth / Grok Build class ≠ console API credits.

---

## Operator ask

Screenshot ~07:02 still showed chrome **6% · 58% behind linear burn**. Message: **"Did it fix it? Didn't seem to."** Prior fix claimed sticky Team SessionToken vs free SuperGrok period ranked personal.

---

## Forensic answers (path traces)

### 1. Is the hot TUI on the new binary?

| Process | Start (local) | `/proc/PID/exe` | Role |
|---------|---------------|-----------------|------|
| **PID 3055710** | ~06:07, re-init ~06:49 | `~/.cargo/bin/grok-oss` **live** (not deleted) | Hot grok-build session `019faf9d-…` |
| PID 2008023 | ~03:30 | `grok-oss` **(deleted)** | iso session, **old inode** |
| PID 2093966 | ~03:34 | `grok-oss` **(deleted)** | bitmagi, **old inode** |
| PID 2100883 | ~03:35 | `grok-oss` **(deleted)** | surmount-server, **old inode** |

Install mtime of prior align binary: **06:49:33** local. Hot session re-inited at **12:49:34 UTC / ~06:49 MDT** with log line proving align code ran. Version in log: `0.2.111`.

**Dogfood fail factor A:** three other TUIs still ran **pre-align binaries** (deleted inode). After ~12:45 UTC, **all** `shell.turn.inference_done` rows were only PID 3055710 (~1.8M tokens in that window). Old sessions were not the post-06:49 burn source, but they matter for any earlier day OAuth climb and for operator multi-session habits: **restart every TUI after `just install`**.

### 2. Did the align log fire?

**Yes.** `~/.grok/logs/unified.jsonl`:

```text
2026-08-08T12:49:34.155Z pid 3055710
auth: aligned SessionToken bearer to free SuperGrok period ranked primary
from_key_prefix=h8CR8FiJmqzA  →  to_key_prefix=X0Zgs3xVormg
```

JWT suffix map (from live `auth.json`):

| Suffix | Principal | team_id |
|--------|-----------|---------|
| `h8CR8FiJmqzA` | **Team** Surmount | `61fab250-…` |
| `X0Zgs3xVormg` | **User** personal | `58c5f686-…` |

Also: base scope on disk after align is **User / personal** (`principal_type=User`, `team_id=58c5…`). Multi-slot still holds Team Surmount.

### 3. Wire-active identity after align?

**Personal User JWT SessionToken** on `https://cli-chat-proxy.grok.com/v1`.

Evidence:

- Align Team → personal (above).
- `auth: cached_token handler set api_key (SessionToken)`.
- Subagent spawn: `auth_type=SessionToken`, `base_url=cli-chat-proxy.grok.com`, `key_prefix=eyJ0eXAi`.
- Billing poll identity flipped **business `61fab…` → personal `58c5…`** at 12:49:35 UTC.
- Config: `preferred_method=oidc`, `auto_use_included_limits=true`, `allow_spend_when_free_period_debit_unproven=true`.

**Not** console ApiKey / `api.x.ai` primary.

### 4. Did free SuperGrok period usage_percent step after ~06:45?

| Window (UTC) | free SuperGrok period `creditUsagePercent` | SuperGrok $ credits | Surmount team OAuth class USD (mgmt) |
|--------------|--------------------------------------------|---------------------|--------------------------------------|
| 12:07–12:49 (Team JWT, pre-align re-init) | **6.0** flat | 10029 flat | ~953 → ~976 (**+$23**) |
| 12:49–13:04 (personal JWT) | **6.0** flat | 10029 flat | ~977 → ~983 (**+$7**) |
| **13:04:03** | **6.0 → 7.0** (one step) | 10029 flat | still climbing |
| After 13:04 | **7.0** | 10029 flat | continues |

Poll rings under `~/.grok/included_poll_history/{58c5…,61fab…}.json` show shared pool (both identities same %). Chrome **6%** at 07:02 was **honest live poll**, not a stale wrong field. By ~07:04 local the poll returned **7%** (weak +1 only).

### 5. Is chrome 6% from poll or stale?

**From live poll.** Unified log `billing: fetched credits config` repeatedly returned `creditUsagePercent: 6.0` (then 7.0). Not client invent.

### 6. Second-order client bugs checked

| Hypothesis | Verdict |
|------------|---------|
| Align only on reconstruct, not request path | BearerResolver re-reads AuthManager after `hot_swap`. After align, reconstruct + sampling use personal. **But** many `AuthManager::new` loads still started on Team **before** first reconstruct. |
| Shared free SuperGrok period pool | **Yes.** Both identity rings always same %. Switching principal alone cannot create a second free SuperGrok period pool. |
| Still ApiKey / console / wrong host | **No** for hot PID after re-init. SessionToken + cli-chat-proxy. |
| Personal JWT still settles as team OAuth | **Observed.** After personal wire-active, Surmount `61fab…` OAuth class **kept climbing** while free SuperGrok period barely moved (one weak +1). Same dual-bill pattern as multi-day C4 under **Team** JWT. |
| `auto_use_included_limits` false | **No.** true in live config. |
| Subagent different auth path | Subagent inherits parent SessionToken + proxy. Same class. |
| Other TUIs on old binary | **Yes (deleted inodes).** Restart required after install. |

---

## Root cause of "didn't seem to"

### What the prior fix did prove

Sticky **Team** SessionToken was a **real** client bug. Align to free SuperGrok period ranked **personal** primary runs, base scope on disk follows personal, billing poll identity becomes personal.

### Why dogfood still looked broken

1. **Free SuperGrok period still does not absorb load** under proven personal SessionToken + cli-chat-proxy. Meter stayed **6%** for ~15 minutes of heavy traffic on the hot PID, then one weak **+1 → 7%**, while **team postpaid OAuth / Grok Build class dollars kept climbing** (~$7 in that window on Surmount `61fab…` management series). SuperGrok dollar credits stayed **$100.29**.
2. That is the same dual-bill / unproven free SuperGrok period debit pattern as multi-day C4 evidence (business JWT eras), now **reproduced with ranked personal JWT on the wire**. Identity selection alone is **not** the full debit fix.
3. Chrome correctly reported server **6%** (then **7%**). Linear burn ~58% is calendar pacing; it is not a client lie. The anger is "work should burn free SuperGrok period harder," which the ledger is not doing.
4. **Secondary:** other long-lived TUIs still ran **old deleted-inode** binaries; always restart all sessions after install.

### Client gap still closed this pass

**Align only on SessionToken reconstruct / prepare_sampling** left a construction window: every `AuthManager::new` loaded sticky Team base first; align ran later (or only on some Arcs). Billing identity and any pre-align traffic could still be Team.

**Next minimal client fix (shipped here):**

1. **`AuthManager::new` with `auto_use_included_limits`:** call `align_to_ranked_free_period_primary` immediately after pin enforce so load never leaves Team wire-active when free SuperGrok period rank prefers another principal.
2. **Path-trace log** on every SessionToken reconstruct: `auth: SessionToken wire bearer for free SuperGrok period path` with `principal_type`, `team_id`, `key_prefix` (no full JWT).
3. Align log now includes `principal_type` / `team_id` / `principal_id`.

This does **not invent free SuperGrok period debit**. It closes the remaining load-time Team window and makes dogfood identity **continuously** observable.

If free SuperGrok period stays flat after restart-all + this install while wire log shows `principal_type=User` / personal `team_id`, the ledger absorption issue is **path-proven client-correct for identity** and the open residual is **server C4 debit** (ticket packages already on disk). Prefer that only with the new wire-bearer log lines attached.

---

## Files

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-shell/src/auth/manager.rs` | Align on `AuthManager::new` when auto_use; richer align log; `session_wire_bearer_trace` |
| `crates/codegen/xai-grok-shell/src/auth/manager_tests.rs` | TDD: auto-use new aligns sticky Team; explicit align when auto_use off; trace asserts |
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` | Log SessionToken wire bearer every reconstruct |

---

## Tests (red→green)

```text
cargo test -p xai-grok-shell --lib free_period
# 21 passed (includes auth_manager_new_auto_use_aligns_sticky_team_base_to_ranked_free_period_primary)

cargo fmt -p xai-grok-shell
cargo clippy -p xai-grok-shell --lib -- -D warnings   # clean
just install   # grok-oss 0.2.111 → ~/.cargo/bin/grok-oss
```

Named contracts:

- `auth_manager_new_auto_use_aligns_sticky_team_base_to_ranked_free_period_primary`
- `align_to_ranked_free_period_primary_switches_sticky_team_base_to_personal` (auto_use false precondition)
- existing free SuperGrok period rank + debit-unproven suite

---

## Dogfood steps (operator)

1. **Quit every** running `grok-oss` / `grok` TUI (including iso / bitmagi / surmount-server). Confirm no `(deleted)` exe:
   ```bash
   pgrep -af grok-oss
   # for each PID: readlink /proc/$PID/exe   # must NOT say (deleted)
   ```
2. Start **only** `~/.cargo/bin/grok-oss` (0.2.111 after this install).
3. Confirm `~/.grok/config.toml`: `auto_use_included_limits = true`, `preferred_method = "oidc"`.
4. Run SuperGrok session turns (not console-primary pin).
5. Watch logs:
   - `auth: aligned SessionToken bearer…` (if base was Team)
   - **`auth: SessionToken wire bearer for free SuperGrok period path`** with `principal_type` + `team_id` every turn
6. Watch free SuperGrok period % (`/limits`, `~/.grok/included_poll_history/`) **and** Surmount team OAuth class (management series).
7. **Pass for client identity path:** wire log shows personal User / `58c5…` every turn; no console primary.
8. **Pass for free SuperGrok period debit:** used % rises with load (or SuperGrok dollar credits after free SuperGrok period is full).
9. **If identity path passes and free SuperGrok period stays flat while team OAuth climbs:** attach wire-bearer log lines + poll history to C4 ticket package (`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md` + multipoll addendum). Do **not** invent % in chrome.

---

## Summary

| Question | Answer |
|----------|--------|
| Why dogfood still looked like 6%? | Server poll honestly returned 6% (then weak 7%). Personal SessionToken was on the wire after align; free SuperGrok period still did not absorb most load; team OAuth still climbed. |
| Hot binary? | Hot PID on live `grok-oss` after re-init; three other sessions still **old deleted** binaries (restart them). |
| Align ran? | Yes, Team → personal at 12:49:34 UTC. |
| Next client fix? | Align on `AuthManager::new` + continuous SessionToken wire-bearer principal logs. Installed. |
| Client path fully disproven for debit? | **Identity path:** proven correct after align (personal SessionToken + proxy). **Debit ledger:** still flat/weak under that path; C4 server residual stays open **with path traces**, not as first shrug. |
