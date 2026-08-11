# Live auth path now — 2026-08-02

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**When:** 2026-08-02 ~06:54 UTC (live `grok-oss limits --json` + same-session logs)
**Mode:** read-only (dumps, config flags, product CLI). No keyring D-Bus archaeology. No secrets.

---

## Executive answer (one screen)

| Question | Answer | Confidence |
|----------|--------|------------|
| Are we on SuperGrok session path right now? | **Yes.** Live sampling = SuperGrok OIDC **business** (Team Surmount). | **Very high** |
| Same as console “Grok Business Usage” page? | **No.** Different meter family. Product explicitly keeps SuperGrok OIDC / `productUsage` separate from console team Business Usage / license seat usage. | **High** |
| Business SuperGrok Heavy “unused”? | **No on the SuperGrok path.** Tier is **SuperGrok Heavy**, included **65% used**, `GrokBuild` **54%** in `productUsage`. The **2026-07-26 org Heavy licence invoice is real**; usage shows on SuperGrok billing, not as zeros on SuperGrok meters. | **Very high** (meters); **medium** (what browser page labels mean if UI still zeros) |
| Stuck on console API burning prepaid while SuperGrok has headroom? | **No.** `console.isLive: false`. Team prepaid **$340** with **0 SPEND** rows in prepaid history. SuperGrok still has **35% included** + **$100.29** extras. | **Very high** |

**One line:** This machine is authenticated and sampling as **SuperGrok Heavy (business OIDC, Team Surmount)**. Console API key is present as failover but not live. Zeros on a **console Grok Business Usage** page are expected for SuperGrok-session work; they do **not** mean Heavy is idle.

---

## 1. Live `limits --json` (this run)

**Binary:** `/home/hunter/Projects/surmount/grok-build/target/release/grok-oss`
(= `~/.cargo/bin/grok-oss`, same inode, mtime 2026-08-01 23:55)

**Command:** `grok-oss limits --json` → exit 0 (also saved `/tmp/limits-live-now.json`)

| Field | Live value |
|-------|------------|
| `liveSampling` | `supergrok_session` |
| `liveSamplingLabel` | Live sampling: SuperGrok session (**business**) |
| `livePrincipalRole` | `business` |
| SuperGrok business/personal `includedUsedPct` | **65.0** (35% remaining) |
| Period | Weekly · next reset **August 3, 19:25** |
| SuperGrok `dollarExtrasUsd` | **100.29** (`dollarExtrasObserved: true`) |
| `sharedUnifiedPool` | **true** |
| `console.keyAvailable` | **true** |
| `console.isLive` | **false** |
| `console.teamPrepaidUsd` | **340.0** |
| `grokBuildUsagePct` on JSON principals | **absent** this surface (see logs §3 — wire has it) |

**Notes from CLI:**

1. Sibling personal principal poll failed: identity prefix `58c5f686-427…` — expired credentials (`PermissionDenied` / no auth context). Matches personal OIDC slot expired ~18h earlier (auth store §4).
2. Honesty note: SuperGrok included % is the **billing poll reading**, not proof of included-limit burn.

**Prior cache** `/tmp/limits-live.json` (2026-08-01 22:22) matches the same shape (session business, 65%, $100.29 extras, prepaid $340). Live re-run did not change the picture.

---

## 2. How to dump limits (product)

```bash
# Preferred: installed / release binary
grok-oss limits --json
# or
/home/hunter/Projects/surmount/grok-build/target/release/grok-oss limits --json

# Workspace equivalent (if rebuilding)
cargo run -p xai-grok-pager-bin --release -- limits --json
```

- Agent-usable, no TUI. Same meters as in-session `/limits`.
- Never prints raw keys/tokens.
- Help text: `grok limits [OPTIONS]` with `--json`.

---

## 3. SuperGrok billing poll (unified log, no secrets)

Source: `~/.grok/logs/unified.jsonl` (tail; latest ~06:53 UTC, pid of live shell).

| Field | Value |
|-------|--------|
| `ctx.role` | `business` |
| `ctx.identity_id` | `61fab250-b2c1-40cf-b5b8-628e673a2eeb` (Team Surmount; same as management team pin) |
| `ctx.subscriptionTier` | **SuperGrok Heavy** |
| `creditUsagePercent` | **65.0** |
| Period | weekly 2026-07-28 → 2026-08-04 UTC |
| SuperGrok prepaid extras (`prepaidBalance.val`) | **10029** cents = **$100.29** |
| `onDemandCap` / `onDemandUsed` | 0 / 0 |
| `isUnifiedBillingUser` | **true** |
| **`productUsage`** | `GrokBuild` **54.0%**, `GrokChat` **11.0%**, `GrokImagine` (no %) |

**Management prepaid (same log window):**
`management prepaid: fetched console team balance` · team `61fab250…2eeb` · `balance_cents: 34000` · `total_val_cents: -34000` → **$340.00**.

**Implication:** SuperGrok Heavy **is** the live subscription tier on the business OIDC principal, and Grok Build is taking most of the included product slice (54 of the 65 top-level %).

---

## 4. Auth store shape (redacted)

File: `~/.grok/auth.json` (keys/tokens redacted; lengths only).

| Slot | Mode | Principal | Team (prefix) | Expires (UTC) | Status at ~06:54 UTC |
|------|------|-----------|---------------|---------------|----------------------|
| base OIDC + `::team::61fab250…` | `oidc` | **Team** Surmount ADMIN | `61fab250-b2c1…` | 2026-08-02T12:28 | **Live ~5.6h left** (refreshed ~06:28 UTC) |
| `::personal` | `oidc` | **User** | `58c5f686-4270…` | 2026-08-01T12:31 | **Expired ~18h** (explains personal billing poll fail) |
| `xai::api_key` | `api_key` | (console inference key) | — | — | Present (key len 84); **not** live sampling |

Email on OIDC slots: operator personal login (not repeated here as needed).
No full tokens, refresh tokens, or API key material in this join.

---

## 5. Config flags (no secrets)

`~/.grok/config.toml` (relevant only):

```toml
[auth]
preferred_method = "oidc"
auto_use_included_limits = true

[endpoints]
management_team_id = "61fab250-b2c1-40cf-b5b8-628e673a2eeb"
```

- No `management_api_key` in config file (key comes from store/keyring; prior wire-capture join already confirmed management fetch works).
- `preferred_method = "oidc"` + `auto_use_included_limits = true` → rank SuperGrok included before $ extras / console when SuperGrok has headroom. Matches live `supergrok_session`.
- UI: theme doge, `permission_mode = always-approve`, default model `grok-4.5`, etc. (not auth-path critical).

---

## 6. Management dumps (cached + prior live join)

| Dump | Role | Result |
|------|------|--------|
| `/tmp/mgmt-prepaid.json` | prepaid balance | `total.val = "-34000"` → **$340**; 17 purchase/auto-purchase rows; **0 SPEND** |
| `/tmp/mgmt-preview.json` | postpaid period preview | Period spend ~**$207.56** (`defaultCreditsIssued`); mostly **Grok Build OAuth** (~$201.76) + **API** (~$5.80); `defaultCredits` **$1500** |
| `/tmp/mgmt-invoices.json` | invoice history | See Heavy licence below |
| `/tmp/grok-join-live-prepaid-wire-capture.md` | prior live wire | Prepaid $340 honest; ~$1317 not on prepaid wire |

### SuperGrok Heavy for Orgs invoice (operator’s 2026-07-26 plan)

From `/tmp/mgmt-invoices.json` (team `61fab250…2eeb`):

| Field | Value |
|-------|--------|
| Invoice # | **HAHH-YYA9-UQ6Q** |
| `createTime` | **2026-07-26T20:44:50Z** |
| Status | PAID |
| Description | **Subscription for SuperGrok Heavy licences** |
| `unitType` | **SuperGrok Heavy for Orgs** |
| Units | 1 |
| Amount | **30000** cents = **$300.00** |

Also present (earlier): invoice **8QSC-Z7LJ-FFAR** (2026-07-23) “Subscription for grok.com licences” / unitType **Grok for Orgs**, $30.

These are **subscription licence lines on team Management billing**, not SuperGrok `productUsage` rows and not console prepaid SPEND.

---

## 7. How product relates SuperGrok Heavy org invoice ↔ OIDC `productUsage` ↔ “Grok Business Usage”

Three distinct families (product law + code + residual):

| Meter family | What it measures | Live evidence now |
|--------------|------------------|-------------------|
| **A. SuperGrok OIDC included + `productUsage`** | Consumer/session SuperGrok weekly included pool; per-product % (`GrokBuild`, `GrokChat`, …) from credits config | **In use:** Heavy tier, 65% overall, Build 54% / Chat 11% |
| **B. SuperGrok $ extras** | Session prepaid extras on SuperGrok (`prepaidBalance`) | **$100.29** on file |
| **C. Console team prepaid / Grok Business Usage class** | Management API team prepaid, postpaid API/OAuth lines, optional usage series — **console product**, not SuperGrok session pool | Prepaid **$340** idle (no SPEND); period postpaid has OAuth Build + some API; product TUI shows prepaid when management configured |

**Code pins (absolute paths):**

- `…/xai-grok-pager/src/views/limits_snapshot.rs` — dual SuperGrok unified pool is **“Also not console.x.ai Grok Business license seat/message usage.”**
- `…/xai-grok-shell/src/extensions/billing.rs` — `productUsage` / `PRODUCT_GROK_BUILD` from SuperGrok credits wire only; never invent.
- `doc/dev/research/console-team-business-usage-meter-2026-07-30.md` — Half B goal is **console team** Business Usage class data; **do not treat SuperGrok Business OIDC as console team prepaid**.
- `RESIDUAL.md` § dual-auth halves — SuperGrok meters (Half A) **and** console Business Usage class (Half B); meters stay distinct.

**Invoice → OIDC:**
Buying **SuperGrok Heavy for Orgs** on the team is the **licence entitlement** that lets Team Surmount OIDC sit on **subscriptionTier: SuperGrok Heavy**. That usage is reported on SuperGrok credits (`creditUsagePercent` / `productUsage`), **not** as a “licence message counter” on the SuperGrok path. The product does **not** map that invoice into a Grok Business Usage UI series (Half B series still open; prepaid balance only for console $).

**If console “Grok Business Usage” shows all zeros:**
Expected when almost all dogfood is **SuperGrok session OIDC** (or OAuth Build billed as postpaid OAuth lines, not licence-seat charts). Zeros there ≠ Heavy unused. Check SuperGrok meters / `grok-oss limits` / grok.com-style SuperGrok usage instead.

---

## 8. Direct answers

### Are we on SuperGrok session path right now?

**Yes.**

Evidence:

1. `limits --json`: `liveSampling=supergrok_session`, `livePrincipalRole=business`.
2. Auth: live Team OIDC Surmount `61fab250…`, `auth_mode=oidc`, ~5.6h to expiry.
3. Billing poll: `subscriptionTier=SuperGrok Heavy`, `role=business`, `identity_id=61fab250…`.
4. Config: `preferred_method=oidc`, `auto_use_included_limits=true`.
5. `console.isLive=false` (console key is failover inventory only for sampling identity).

### Is that the same as “Grok Business licenses Usage” page?

**No.**

Evidence:

1. Product snapshot comment + research doc: SuperGrok unified pool / OIDC **≠** console Grok Business **license seat/message** usage.
2. Residual: Half A (SuperGrok) vs Half B (console Business Usage class) are **both** wanted and **must not** be collapsed.
3. Live: SuperGrok path has non-zero included/`productUsage`; console prepaid ledger has **no SPEND**; Management “Business Usage” charts (POST usage series) are **not** what drives SuperGrok sampling.
4. The **Heavy for Orgs** line is a **paid subscription invoice**, not a usage chart. It entitles Heavy on the OIDC business principal; usage then appears on SuperGrok billing.

### If Business Usage zeros are expected when using SuperGrok Heavy OIDC (or only API keys for other tools)?

**Yes, expected for SuperGrok-session work.**

- Live sampling is SuperGrok OIDC Heavy business → SuperGrok meters move; console licence-usage charts can stay flat.
- Console **inference** key is available but **not** live sampling → pure API-key burn is not the current primary path either.
- Caveat: Management **postpaid preview** still shows **Grok Build OAuth** period lines (~$202). That is **team postpaid / OAuth product accounting**, still not the same as a “Business licences Usage” zero panel, and not SuperGrok included % itself. Do not read postpaid OAuth lines as “we never used Heavy.”

### Wrongly on console API burning prepaid while SuperGrok headroom exists?

**No — not the failure mode right now.**

| Check | Evidence |
|-------|----------|
| Live principal | SuperGrok session business, not console |
| SuperGrok headroom | 35% included remaining + $100.29 extras |
| Console prepaid | $340 remaining; prepaid history **purchases only**, **0 SPEND** |
| Config ranking | OIDC preferred + auto_use_included_limits |

Opposite of “stuck on console draining prepaid while SuperGrok free.” SuperGrok is primary; prepaid is sitting.

---

## 9. Small caveats (honesty)

1. **Included % ≠ proven burn:** product honesty note still applies; 65% is the billing poll. With `productUsage` Build 54% / Chat 11% and active dogfood, non-zero use is still well supported.
2. **Personal SuperGrok slot expired** — dual-row shows same unified % from successful business poll; personal poll fails until re-login. Unified pool flag remains true.
3. **JSON `grokBuildUsagePct` field** not present on this CLI JSON dump even though wire `productUsage` has GrokBuild 54% in logs — surface gap for scripts; log/TUI path has the data.
4. **management_team_id equals SuperGrok OIDC team id** on this host (`61fab250…`). Research says do not *assume* equality in general; here they match for Surmount.
5. Dumps under `/tmp/mgmt-*.json` are from earlier same-day/prior session; prepaid total and Heavy invoice are stable with live log re-fetches of prepaid $340.

---

## 10. Sources (absolute)

| Path | Use |
|------|-----|
| Live CLI | `…/target/release/grok-oss limits --json` → `/tmp/limits-live-now.json` |
| Cache | `/tmp/limits-live.json`, `/tmp/mgmt-prepaid.json`, `/tmp/mgmt-preview.json`, `/tmp/mgmt-invoices.json` |
| Prior join | `/tmp/grok-join-live-prepaid-wire-capture.md` |
| Log | `~/.grok/logs/unified.jsonl` |
| Config / auth shape | `~/.grok/config.toml`, `~/.grok/auth.json` (redacted) |
| Product | `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`, `…/limits_cmd.rs`, `…/xai-grok-shell/src/extensions/billing.rs` |
| Research / residual | `doc/dev/research/console-team-business-usage-meter-2026-07-30.md`, `RESIDUAL.md` (dual-auth halves) |

**No product code edits. No secrets printed.**
