# Forensic: "Seems to be very broken right now" (2026-08-08)

**Session:** `019faf9d-ef93-7d93-b34b-9f19b6345613` (grok-build)  
**Forensics time:** 2026-08-08 ~12:03–12:06 UTC  
**Product code fixed this turn:** none (no new red client bug; wrong live binary + already-shipped tree fixes)

---

## Executive summary

| Claimed / feared | Status |
|------------------|--------|
| Flat free SuperGrok period debit **blocks** turns by default | **Fixed in tree + installed `grok-oss`**. Config also has allow=true. Earlier hard blocks were on **pre-default-flip** `0.2.111` (old message text). |
| `/rebuild` re-fires old prompts (stale `canceled_turn_resume`) | **Fixed in tree + installed `grok-oss`** (stale-marker gate on session load). |
| Free SuperGrok period stuck ~6% | **Server C4**, not a client spend-order bug (prior multipoll / ticket). Do not invent free-period debit. |
| Killall resume history recovery races | Prior implement slices claimed; not the live hot-path failure now. |
| **"Very broken" right now** | **Hot session is running official `grok` 1.0.0**, not Surmount `grok-oss`. That binary has **zero** Surmount debit-guard / stale-resume product strings. |

**Dogfood fix for the operator (no multipoll):** full quit of the hot TUI, then reopen with **`grok-oss`**, not `grok`. Prefer deleting or ignoring the leftover cancel marker if you want a clean idle load.

---

## 1. Live binaries and processes

### Installed Surmount product

| Item | Value |
|------|--------|
| Path | `/home/hunter/.cargo/bin/grok-oss` |
| Version | `grok-oss 0.2.111 (c87f66a61d94)` |
| mtime | 2026-08-08 05:52:26 -0600 |
| Git HEAD | `c87f66a61d94` (`c87f66a fixes`) — **matches** binary embed |
| Tree default debit allow | `default_allow_spend_when_free_period_debit_unproven() -> true` |

### PATH `grok` (trap)

| Item | Value |
|------|--------|
| `which grok` | `/home/hunter/.grok/bin/grok` |
| Symlink | `-> ../downloads/grok-1.0.0-linux-x86_64` (set 2026-08-08 05:12) |
| Version | `grok 1.0.0 (3cd0d0cbce)` **official download** |
| Surmount debit guard in binary | **absent** (`strings` count of `allow_spend_when_free_period_debit_unproven` = **0**) |

### Live processes at forensics

| PID | exe | cwd / session | Notes |
|-----|-----|---------------|--------|
| **2809478** | `/home/hunter/.grok/downloads/grok-1.0.0-linux-x86_64` | grok-build / **019faf9d…** | **Hot session.** cmdline: `grok --resume 019faf9d-…` |
| 2008023 | `~/.cargo/bin/grok-oss` **(deleted)** | ai/iso | Pre-05:52 install; still running old inode |
| 2093966 | `grok-oss` **(deleted)** | bitmagi | same |
| 2100883 | `grok-oss` **(deleted)** | surmount-server | same |

`active_sessions.json` lists the hot session opened at `2026-08-08T11:56:47Z` under PID 2809478 (official 1.0.0).

---

## 2. Config

`~/.grok/config.toml`:

```toml
[auth]
allow_spend_when_free_period_debit_unproven = true
```

Env `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN` unset (config / default apply).

So even on a **new** `grok-oss` with the old hard-block default, config would allow. Live hard blocks at 11:11–11:13 were **before** the default flip *and* used the **old** error copy (told operator to set `= true`, not the post-fix "Hard block is on because … = false" wording). Installed `grok-oss` embeds the **new** opt-in hard-block message.

---

## 3. Hot session disk state

Path:

`~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fsurmount%2Fgrok-build/019faf9d-ef93-7d93-b34b-9f19b6345613/`

### `canceled_turn_resume.json` (present)

```json
{
  "prompt_text": "??? [Image #1]",
  "prompt_id": "882cb6c3-0e99-4174-a774-8001852e204f",
  "canceled_at": "2026-08-08T11:56:26.569480258+00:00",
  "reason": "user_cancel"
}
```

mtime 05:56 local (~11:56 UTC). Matches log: `shell.cancel.received` / `ctrl_c` on prior `0.2.111` PID 1415427, then **reopen on official 1.0.0**.

Official 1.0.0 did **not** overwrite this marker for the next turn (no Surmount eager-write path, or different product). Marker still points at the **older** cancel prompt while chat continued with later user text.

### Chat history (recent user turns)

Includes multiple `??? [Image #1]` re-entries (rebuild/refire era), then:

- interject about rebuild re-firing old prompt  
- **`Seems to be very broken right now`** (prompt_index 344)

### Live turn (not a phantom image re-fire)

Logs for PID 2809478 / ver **1.0.0**:

| UTC | Event |
|-----|--------|
| 11:56:26 | ctrl_c cancel on prior `0.2.111` (marker written) |
| 11:56:39 | `session.load.start` on **1.0.0** |
| 11:56:47 | `session.load.done` |
| 11:57:07 | `prompt.drain` **prompt_len=33** (= length of `Seems to be very broken right now`) |
| 11:57–12:02 | inference retries (proxy error, then HTTP 502) |
| 12:03+ | turn progressing (tools / subagent) |

So the current work is the operator’s “very broken” message on the **wrong binary**, after network flakiness. Not an infinite image auto-refire in this open.

---

## 4. Recent errors (unified.jsonl)

### Hard debit block (historical on this session)

Three `agent response failed` lines, ver **0.2.111**, sid hot session:

- 11:11:11, 11:11:23 (PID 1982020)  
- 11:13:58 (PID 1401298)  

Error body: *Blocked: free SuperGrok period limits are not debiting (flat poll)… Set `[auth] allow_spend_when_free_period_debit_unproven = true`…*  
(old default=false / pre-config-or-pre-flip binary)

**No further debit-block errors after 11:13.** Default-flip implement finished ~11:31 (log subagent complete). Config now true; installed binary defaults allow.

### Other noise (not the primary “broken”)

- Repeated `auth 401 attribution` / `is_stale_snapshot` on long-lived deleted-binary sessions  
- `billing: upstream request failed` (proxy fetch)  
- Inference retries / 502 on official 1.0.0 hot session  

---

## 5. Code verify: claimed fixes **are** in tree (and in installed `grok-oss`)

### 5a. `default_allow_spend_when_free_period_debit_unproven == true`

- `crates/codegen/xai-grok-shell/src/auth/config.rs` — const returns **`true`**; empty-config tests assert allow.  
- Installed binary strings: dual-auth “turns allowed (default)” and hard-block only when explicitly false.

### 5b. Stale marker gate on session load (completed primary)

- `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` ~1149–1160: if marker present **and** `last_primary_user_turn_completed_in_replay` **and** not mid-work → **drop marker, no SendPrompt**.  
- Log string present in `grok-oss` binary: `canceled_turn_resume: dropping stale marker after completed primary turn (no mid-work)`.  
- Unit coverage in `dispatch/tests/turn.rs` (stale marker must not fire / must clear).

### 5c. `/rebuild` does not force re-fire of completed prompts

- Rebuild mid-turn still cancel-resumes **running** turns only (`rebuild.rs`).  
- Relaunch reuses **same session load path** as above; completed primary + no mid-work → idle.  
- Prior report: `.agents/reports/impl-rebuild-no-refire-old-prompt-2026-08-08.md`.

### 5d. Free period ~6% / C4

Prior multipoll and C4 ticket material remain the authority. Client must **not** invent free SuperGrok period debit. Settlement can still move under SuperGrok session while free-period % is flat.

---

## 6. What is broken vs already fixed but old / wrong binary

| Symptom | Root | Fixed? | Needs |
|---------|------|--------|--------|
| Turns blocked on flat free-period poll | Default was hard-block | **Yes** (tree + install + config) | **Quit** old processes; use **new** `grok-oss` |
| Rebuild re-fires finished prompt | Stale marker always applied | **Yes** (tree + install) | Same; do not resume with official `grok` |
| Hot TUI “very broken” / missing Surmount limits stack | **Running official 1.0.0** via `~/.grok/bin/grok` | N/A (wrong product) | Quit PID 2809478; start **`grok-oss --resume 019faf9d-…`** (or cwd open) |
| iso/bitmagi/surmount-server still on deleted exe | Never restarted after 05:52 install | Process hygiene | Full quit those TUIs so they pick up new binary |
| Free period stuck ~6% | Server C4 | Not client-fixable | Existing ticket / multipoll evidence only |
| Leftover `canceled_turn_resume.json` = `??? [Image #1]` | Real user_cancel at 11:56; official binary didn’t supersede marker | Gate drops if primary completed + idle on **grok-oss** load | Optional: remove file before clean reopen if you want zero auto-resume risk while mid-chaos |

---

## 7. What this turn fixed in product code

**Nothing.** No new red Surmount contract found that needs a code edit. Installed `grok-oss` already matches HEAD with debit default + stale gate. Re-running multipoll was not used as the answer.

---

## 8. Dogfood: what the operator should do

**No multipoll required for this diagnosis.**

1. **Full quit** the hot grok-build TUI (PID 2809478, official `grok 1.0.0`). Do not leave it running and expect Surmount fixes.  
2. Optionally remove the stale marker if you want a guaranteed idle load:
   ```bash
   rm -f ~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fsurmount%2Fgrok-build/019faf9d-ef93-7d93-b34b-9f19b6345613/canceled_turn_resume.json
   ```
   (On fixed `grok-oss`, a completed primary with no mid-work also clears it on load.)  
3. Reopen with the **Surmount** binary only:
   ```bash
   grok-oss --resume 019faf9d-ef93-7d93-b34b-9f19b6345613
   ```
   or from the repo: `just install` if you rebuild again, then **`grok-oss`**, never bare `grok` while `~/.grok/bin/grok` points at downloads 1.0.0.  
4. Confirm chrome: `grok-oss --version` should print `0.2.111 (c87f66a…)` (or newer after install), **not** `grok 1.0.0`.  
5. Other workspaces (iso, bitmagi, surmount-server): full quit so they leave **deleted** exes and pick up the same install.  
6. Free SuperGrok period % still flat is **C4 server honesty**, not a reason to re-block turns (allow is default + config true).

**Avoid:** using `grok` on PATH for dogfood of Surmount work; auto-update rewrote `~/.grok/bin/{grok,agent}` to official 1.0.0 at 05:12.

---

## Evidence index (absolute paths)

- Binary: `/home/hunter/.cargo/bin/grok-oss`  
- Official trap: `/home/hunter/.grok/bin/grok` → `/home/hunter/.grok/downloads/grok-1.0.0-linux-x86_64`  
- Config: `/home/hunter/.grok/config.toml`  
- Session: `/home/hunter/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fsurmount%2Fgrok-build/019faf9d-ef93-7d93-b34b-9f19b6345613/`  
- Logs: `~/.grok/logs/unified.jsonl`  
- Prior fix reports:  
  - `.agents/reports/impl-unblock-flat-poll-default-2026-08-08.md`  
  - `.agents/reports/impl-rebuild-no-refire-old-prompt-2026-08-08.md`  
  - `.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`  
