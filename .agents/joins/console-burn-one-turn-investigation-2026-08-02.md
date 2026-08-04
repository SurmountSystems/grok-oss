# Join: console Usage +$0.01 one-turn investigation (2026-08-02)

**Mode:** read-only evidence. No product edits. No secrets.
**Workspace:** `/home/hunter/Projects/surmount/grok-build`
**When investigated:** 2026-08-02 ~01:06–01:10 MDT (07:06–07:10 UTC)
**Operator concern treated as validated:** console team **API Usage** moved under dogfood while SuperGrok included still showed headroom. That is the pain meter for credits burn. Do not dismiss as “wrong meter, ignore.”

---

## One-line

**Most likely the +$0.01 is this product’s SuperGrok session traffic (SessionToken → `cli-chat-proxy.grok.com`) settling as team-side “Grok Build OAuth” dollars on console Usage — not a live console ApiKey hop on the dogfood process; main sampling stayed SuperGrok the whole window.**

---

## Operator evidence (validated)

| Field | Value |
|-------|--------|
| Page | console.x.ai team **API Usage** (`…/usage`, **not** `…/grok-business/usage` licenses) |
| Team | `61fab250-b2c1-40cf-b5b8-628e673a2eeb` (Surmount) |
| Before turn | Spend **$547.87** |
| After turn (~01:04 AM local) | Spend **$547.88** |
| Delta | **+$0.01** |
| Week shape (same page) | Text ~$332.63 + Grok Build ~$214.41 (+ tiny image/voice) |
| SuperGrok included (product) | Still ~**65%** used (~35% headroom), SuperGrok $ extras **$100.29** flat |

Local time for this host: **America/Denver (MDT, UTC−6)**. Operator ~01:04 AM local ≈ **07:04 UTC**.

---

## Evidence table (redacted; no secrets)

| Time (UTC) | Source | Finding | Implication |
|------------|--------|---------|-------------|
| Live re-run ~07:07 | `grok-oss limits --json` | `liveSampling=supergrok_session`, `livePrincipalRole=business`, `console.isLive=false`, `console.keyAvailable=true`, prepaid **$340**, included **65%**, extras **$100.29** | Primary sampling identity is SuperGrok session, not console key |
| Log file span 2026-08-01T23:06Z → 2026-08-02T07:0xZ | `~/.grok/logs/unified.jsonl` (~16k lines, this dogfood slice only) | Every production **subagent spawn credentials** row: `auth_type=SessionToken`, `base_url=https://cli-chat-proxy.grok.com/v1`, `key_prefix=eyJ0eXAi…` (JWT), `auth_method_id=cached_token` | Subagents inherit SuperGrok proxy, not `api.x.ai` + ApiKey |
| Whole file | spawn summary | **56** SessionToken+proxy spawns; **0** live ApiKey spawns | No live console-key subagent path in this log |
| Whole file | `base_url` aggregation | **224** `cli-chat-proxy.grok.com`; **5** `api.x.ai` only in kill-switch **block** test lines | No successful live hop log to console inference host |
| ~06:40Z | cargo/test pids | `auth: kill switch blocked a first-party API key… base_url=https://api.x.ai/v1`; ApiKey rows with `auth_method_id=test` | Test noise, not dogfood burn |
| Auth method selection (11× live process starts) | unified log | Always `default_auth_method_id=cached_token`, `has_cached_token=true`; live rows `has_external_api_key=false` | Product prefers session over console |
| Config | `~/.grok/config.toml` | `preferred_method="oidc"`, `auto_use_included_limits=true`, `management_team_id=61fab250…` | Limits-first config intent matches live path |
| Auth store | `~/.grok/auth.json` (shapes only) | Team OIDC live (JWT, expires ~12:28Z); personal OIDC **expired**; `xai::api_key` present (len 84, `xai-` shape) | Console key inventory exists; not live sampling |
| Live process env | `/proc/<grok-oss-pid>/environ` | **No** `XAI_API_KEY` on dogfood `grok-oss` (pid ~3138654); **has** `OPENROUTER_API_KEY` | Live binary not sampling from env console key |
| Host shell | fish global export | `XAI_API_KEY` **set** (len 84), same key as auth.json by equality check | Other tools / shells can burn console; this product process did not inherit it |
| ~07:04Z (≈01:04 MDT) | inference_done | Active SuperGrok turns: multi-loop inference, large prompt caches (tens of k–100k tokens), subagent spawn SessionToken+proxy | Time-aligned with operator screenshot window; traffic is SuperGrok path |
| Management postpaid (cached `/tmp/mgmt-preview.json`, same team) | line items | **Grok Build OAuth** ~**$201.72** vs **API** models ~**$5.84** (+ STT ~$0, storage ~$0) of ~$207.56 period `defaultCreditsIssued` | Team **dollar** burn is mostly **OAuth Grok Build**, not ApiKey. SuperGrok CLI path **does** post dollars on team billing surfaces |
| Management prepaid | balance | **$340** remaining, **0 SPEND** rows in prior dump | Usage $ is **not** the prepaid ledger; likely **defaultCredits** / postpaid-class (open reconciliation, same as F1b) |
| Image gen code | `prepare_image_gen_config` | **Both** session and BYOK go to `xai_api_base_url` (default `api.x.ai`) with sampling bearer | Real console-edge bypass for **Imagine**; week Image & Video only **$0.83**, so not the week bulk |
| Voice | mgmt line + code | STT can hit `api.x.ai` / `wss://api.x.ai`; week Voice **&lt;$0.01** | Unlikely primary for $547; possible micro-cent contributor |
| Log volume this file | inference_done | **1126** completions, ~**103M** tokens in ~8h slice | Week Usage claims ~**1.14B** tokens / **57k** reqs → multi-day / multi-client; this process is a large dogfood contributor, not the only possible one |

---

## What most likely spent the +$0.01

### Ranked for the one cent

| Rank | Hypothesis | Likelihood | Why |
|------|------------|------------|-----|
| **1** | **This product SuperGrok OAuth (Grok Build) settlement on console Usage $** | **High** | Live path is 100% SessionToken+proxy; management postpaid is **~97% OAuth Grok Build** dollars; dogfood turn at 07:04Z is exactly that path. Console Usage **Grok Build** line (~$214) lines up with OAuth class, not ApiKey-only. |
| **2** | **Delayed / batched chart tick** for earlier OAuth (or same turn lag) | **Medium–high** | Usage UI is not a per-request ledger. +$0.01 can be a late micro-settle while list-rate token volume for that minute is much larger if fully priced. Does **not** clear product of OAuth→Usage coupling. |
| **3** | **True ApiKey / `api.x.ai` inference from some client on the team key** | **Low for this process now; medium team-wide for the week** | No live ApiKey sampling logs this window; env key exists for other processes; week still has ~$5–6 API-class postpaid + large historical dogfood windows. |
| **4** | **Image/voice direct-to-`api.x.ai`** with session or key | **Low for this cent** | Code can hit `api.x.ai`; week image/voice dollars are tiny. |
| **5** | **Management API polls** | **None** | Management is not inference Usage; prepaid fetch only. |

### Ranked for the larger ~$547 week (same team Usage)

| Rank | Source | Likelihood | Evidence |
|------|--------|------------|----------|
| **1** | **This product’s SuperGrok session dogfood billed as team “Grok Build OAuth” (and related Text aggregation)** | **High** | Postpaid OAuth-dominant; 1000s of SuperGrok turns; Design A correctly keeps console key **out** of primary chain while included has headroom — yet **OAuth still creates team $** |
| **2** | **Console API key traffic** (this product on older configs / hops, or other tools using `XAI_API_KEY` / store key) | **Medium** | ~$5–6 API-class in one postpaid snapshot is small vs $547; week Text $332 may mix product buckets; env key is exported in fish |
| **3** | **Other machines / teammates / scripts** on same team key | **Open / medium** | Team Usage is team-wide; this host’s log is one process slice |
| **4** | **Imagine / STT** | **Low for bulk** | Small week lines |

**Critical honesty (F1a + F1b join):** SuperGrok included **% stayed flat at 65%** while team Usage dollars moved a lot. Auth path “on SuperGrok” is **necessary but not sufficient** for limits-first ideal. Operator pain on console Usage is **correct** even when `console.isLive=false`.

---

## Remaining console-edge paths (product), ranked

| Rank | Path | Hits console Usage $? | Status in this dogfood |
|------|------|------------------------|------------------------|
| 1 | **Main + subagent sampling via SuperGrok OAuth → team OAuth/Grok Build lines** | **Yes (team $ via OAuth class)** | **Active** (SessionToken + proxy). **Primary suspect.** |
| 2 | **Console key in resolve chain** when included exhausted / sticky exhaust / `preferred_method=api_key` / Design A off | Yes (API class) | **Not active** now: Design A + headroom → console omitted; `isLive=false` |
| 3 | **Env `XAI_API_KEY` outside product** (shells, other CLIs, scripts) | Yes | **Present on host**; **not** in live `grok-oss` environ |
| 4 | **Image / video gen** always `xai_api_base_url` (default `api.x.ai`) with sampling bearer | Yes (Image & Video bucket) | Code path real; little spend this week |
| 5 | **Voice STT** `api.x.ai` / `wss://` | Yes (Voice) | Tiny |
| 6 | **BYOK / custom model / OpenRouter** | OpenRouter = different vendor; BYOK own key | OpenRouter env set; not console.x.ai team Usage |
| 7 | **Title / recap / embeddings** | Would depend on host | No title/recap/embed log hits in this file; recap disabled in config |
| 8 | **Management prepaid poll** | No inference spend | Active; not Usage $ |
| 9 | **Kill-switch / enterprise strip** | Blocks console key | Observed only in tests this window |

---

## Config / env / auth (redacted)

| Item | Value |
|------|--------|
| `[auth] preferred_method` | `oidc` |
| `[auth] auto_use_included_limits` | `true` |
| Model base_url overrides in config | **None** |
| `XAI_API_KEY` in fish | **SET** (len 84, same as auth.json key) |
| `XAI_API_KEY` in live `grok-oss` process | **unset** |
| `OPENROUTER_API_KEY` | SET on host and live process (not console Usage) |
| Auth slots | Team OIDC live; personal OIDC expired; `xai::api_key` present |

---

## Concrete next fix (if this product is “guilty”)

**Guilt model to use:** product is guilty of **team dollar burn on SuperGrok OAuth while included headroom still shows**, not of “secretly using the console API key on every turn.” Design A did its job for ApiKey chain; the pain meter still moves.

### Smallest product change (recommended order)

1. **Honesty / observability (smallest, ship first)**
   - Surface plain language: **“SuperGrok session can still move console team API Usage dollars (OAuth / Grok Build class) even when console key is not live.”**
   - Wire into `/limits` notes + doctor + residual, not only F1b plan prose.
   - Optional log line once per turn: `billing_path=supergrok_oauth_proxy` vs `console_api_key` (host already known at resolve).

2. **Prove / disprove included absorption (product + server)**
   - Red/green **cannot** invent server debit. What product can do: time-series export of `creditUsagePercent` / `productUsage` / Usage $ under a controlled load window; fail C4 loudly when flat under load.
   - If OAuth Grok Build is **supposed** to debit included weekly first: file/track **upstream billing attribution** — product alone cannot move their ledger.

3. **Console key hygiene (local, immediate)**
   - Unset fish global `XAI_API_KEY` (or stop exporting) so non-product tools cannot burn the team key.
   - Keep key in auth store only if failover is desired; Design A already strips it while headroom remains.

4. **Image/voice direct host (next code slice if operator wants zero `api.x.ai` under headroom)**
   - Route Imagine/STT through the same policy as sampling (prefer proxy/session metering; document if server **requires** `api.x.ai`).
   - Not the week bulk, but a real Design A hole.

5. **Do not “fix” by forcing more ApiKey**
   - Wrong direction for limits-first.

### If other clients are likely: how to prove

| Step | How |
|------|-----|
| A | **Freeze product dogfood 30–60 min** (quit `grok-oss`); watch console Usage. If cents still climb → other clients. |
| B | **Unset/export-block `XAI_API_KEY`** host-wide; rotate console inference key in console UI; re-run only SuperGrok session. If API-class postpaid lines die but OAuth lines continue → this product OAuth is the bulk. |
| C | **Re-fetch** `GET …/postpaid/invoice/preview` (management key): compare **Grok Build OAuth** vs **API grok-*** line totals before/after a pure SuperGrok session window. |
| D | **Team audit:** any other machines, CI, Cursor/continue, scripts with the team key; Management API key users. |
| E | **Correlation:** Usage chart “Grok Build” vs product log SessionToken volume; “Text”/API vs ApiKey logs (`auth_type=ApiKey`, `api.x.ai`). |

---

## Relation to prior joins / plan

| Artifact | Role |
|----------|------|
| `.agents/joins/console-api-usage-547-evidence-2026-08-02.md` | $547.87 pin; F1b |
| `.agents/joins/live-auth-path-now-2026-08-02.md` | Live SuperGrok path; `console.isLive=false` |
| `.agents/plans/limits-first-ideal-2026-08-02.md` | C1–C7, F1a/F1b; this join tightens **one-turn** attribution |
| Management `/tmp/mgmt-preview.json` | OAuth-dominant team $ (~$201 OAuth vs ~$6 API) |

**Plan criteria impact:**

| Criterion | After this investigation |
|-----------|---------------------------|
| **C1** (auth path SuperGrok) | **Still pass** for this process |
| **C3** (no silent **console key** in chain under headroom) | **Pass** for main/subagent sampling; **partial fail** if C3 is read as “no console Usage $ movement” — OAuth dollars still move |
| **C4** (included weekly absorbs) | **Still fail / unproven** (flat 65% + Usage $ up) |
| **F1b** | **Strengthened** with one-turn +$0.01 and OAuth-as-primary-dollar mechanism |

---

## Bottom line

Operator was right to treat the +$0.01 as real burn signal.
This host’s dogfood process is **not** secretly primary-sampling with the console API key right now.
It **is** almost certainly contributing team console Usage dollars via **SuperGrok OAuth / Grok Build** billing, while included % stays flat — that is the limits-first gap that hurts, and it is worse than a one-off ApiKey mis-route because the “correct” path still spends.
