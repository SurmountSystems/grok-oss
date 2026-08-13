# Live check: free SuperGrok period limits vs credits chrome

**Date:** 2026-08-07
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Mode:** read-only (no product edits, no git, no secret dumps)

## Verdict (plain English)

**You are on free SuperGrok period right now.** Live sampling is SuperGrok session (OIDC / SessionToken via cli-chat-proxy), free-period used is about **6%**, and free SuperGrok period still has room. Showing free-period **%** chrome is correct Design A behavior.

**This is not the “on credits while still showing free-period %” bug.** That bug is for console live or SuperGrok free period full while SuperGrok dollar extras drive after-burner spend. Neither is the live path in current dogfood.

**What the status bar should show on this path:** free SuperGrok period used, e.g. **`6%`** (optionally with a short pacing chip). It should **not** show `console · $…` and **not** show `SuperGrok extras · $…` while free period is under 100%, even though SuperGrok dollar extras and console team prepaid both exist on the account as other meters.

---

## Binary / install (filesystem; no `which` shell in this agent)

| Path | Notes |
|------|--------|
| `/home/hunter/.cargo/bin/grok-oss` | Present (typical `just install` dogfood target) |
| `/home/hunter/Projects/surmount/grok-build/target/release/grok-oss` | Present (local release build) |
| `/home/hunter/.grok/bin/grok` | Binary; content resolves to downloads build **0.2.118** (`grok-0.2.118-linux-x86_64`) |
| `/home/hunter/.grok/downloads/grok-0.2.118-linux-x86_64` | Present |
| Tree `SOURCE_REV` | `124d85bc5dc6e7805560215fcc6d5413944920e1` |

**CLI note:** This explore agent has no shell tool, so `grok-oss limits --json` / `grok limits --json` were **not** executed in-process. Live state below is from product dogfood logs and on-disk billing poll history (same meters `limits --json` would summarize). Operator can still re-check with:

```bash
which grok-oss grok
ls -la "$(which grok-oss)" "$(which grok)" 2>/dev/null
grok-oss --version 2>/dev/null || grok --version
grok-oss limits --json
```

---

## Live meters (non-secret; from dogfood)

**Sources (2026-08-07, last samples ~20:44 UTC):**

- `~/.grok/logs/unified.jsonl` (`billing: fetched credits config`, `management prepaid`, subagent spawn credentials)
- `~/.grok/included_poll_history/61fab250-b2c1-40cf-b5b8-628e673a2eeb.json`
- `~/.grok/config.toml` auth flags
- `~/.grok/exhausted_credits/` (empty)
- Active sessions include grok-build, bitmagi, surmount-server (PIDs present; no tokens)

| Meter / flag | Live reading | Meaning |
|--------------|--------------|---------|
| Auth path | SuperGrok **SessionToken**, base `https://cli-chat-proxy.grok.com/v1` | SuperGrok session sampling, not console API key host |
| Live SuperGrok principal (poll identity) | business team id `61fab250-…` (role business in logs) | Active SuperGrok slot for billing polls |
| Free SuperGrok period used | **6.0%** (`creditUsagePercent`) | Weekly included pool has room |
| Free SuperGrok period window | 2026-08-04 → 2026-08-11 (weekly) | Current billing week |
| SuperGrok dollar extras | **10029 cents** (`prepaidBalance`) | ~$100.29 on account; **not** the live compact driver while included &lt; 100% |
| Console team prepaid | **34000 cents** | ~$340 team Management prepaid (side meter while SuperGrok session is live) |
| `preferred_method` | `oidc` | Not pinned to console API key |
| `auto_use_included_limits` | `true` | Prefer free SuperGrok period before credits |
| Exhaust sticky (`exhausted_credits/`) | **empty** | No durable “SuperGrok out of free period” memo |

Inference spawn credentials in the same window consistently use SessionToken + cli-chat-proxy (SuperGrok session). That matches free-period driving under auto_use + room remaining.

---

## Code path still matches Design A

### Compact helper (`credit_bar.rs`)

`compact_meter_text_for_live_identity` still documents and implements Design A:

| Live spend path | Compact status meter |
|-----------------|----------------------|
| Console live | `console · $N` or honest gap. Never bare SuperGrok free-period `%` / `...%` |
| SuperGrok live, free period has room (`included < 100%`) | Free-period used `%` |
| SuperGrok live, free period full + SuperGrok `$` extras remain | `SuperGrok extras · $N` (not bare `100%`) |
| SuperGrok live, free period full, no extras | `100%` |
| SuperGrok live, cold included | `...%` |

`credit_bar_line_for_session` builds SuperGrok-primary chrome through that helper and skips free-period pacing chip on the extras `$` path.

### Status bar (`agent_view/render.rs`)

Before paint:

1. Sticky console pin via `supergrok_out_of_allowance_with_console_ready` (same idea as footer).
2. Console branch → `compact_meter_text_for_live_identity(ConsoleKey, …)`.
3. SuperGrok branch → `credit_bar_line_for_session` (which uses the helper with SuperGrok session + prepaid extras cents).

### Unit contracts still present

Named tests remain in `credit_bar.rs`:

- console live ≠ SuperGrok chrome
- SuperGrok full + extras → dollars not free-period `%`
- SuperGrok free-period room → `%` even when extras exist on the account
- SuperGrok full, zero/no extras → `100%`

---

## Apply Design A to *this* live snapshot

| Input | Value |
|-------|--------|
| Live identity | SuperGrok session (not console) |
| Free period known | yes |
| Free period used | 6% (&lt; 100%) |
| SuperGrok extras cents | 10029 (&gt; 0, but ignored for compact while free period has room) |

**Expected compact status text:** `6%` (plus optional pacing).
**Not expected:** `SuperGrok extras · $100.29` or `console · $340`.

So: free-period chrome **yes**; “still on credits bug” **no** for current dogfood.

---

## Caveats

1. **No live `limits --json` process in this agent.** If the operator wants a wire-level JSON dump after rebuild, run the commands above in a real TTY.
2. **Running sessions may still be an older binary** until restarted after rebuild. Tree has Design A; dogfood process must be the rebuilt `grok-oss` to paint the fixed chrome for the *credits* paths (console live / full + extras). Current path already wanted free-period `%` even before the fix.
3. **Team postpaid / OAuth class burn** can still move on console Management while sampling is SuperGrok session. That is a separate honesty / settlement topic; it does not by itself flip the compact meter to console `$` under Design A.

---

## Bottom line

| Question | Answer |
|----------|--------|
| Free SuperGrok period driving now? | **Yes** (~6% used, SuperGrok session live) |
| On credits while wrongly showing free-period %? | **No** (not the live path) |
| Status bar should show | **Free-period `6%` chrome** |
| Design A still in tree? | **Yes** (`compact_meter_text_for_live_identity` + status sticky pin) |
