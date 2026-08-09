# Authentication

Grok supports several authentication methods, including interactive browser login, enterprise single sign-on (SSO), and headless CI/CD runners.

---

## Browser Login (Default)

On first launch, Grok opens your browser to authenticate with grok.com:

```bash
grok
```

Grok stores credentials in `~/.grok/auth.json` and reuses them across sessions. Grok refreshes access tokens automatically in the background. When a token can't be refreshed, Grok prompts you to sign in again. Credentials without a server-provided expiry fall back to a 30-day lifetime.

### Credential storage

Tokens in `~/.grok/auth.json` (and MCP OAuth tokens in `~/.grok/mcp_credentials.json`) are written with owner-only permissions (`0600` on Unix). Anyone with filesystem access to those paths can use the credentials, so:

- Prefer full-disk encryption (FileVault, BitLocker, LUKS, or equivalent).
- Do not copy `auth.json` or `mcp_credentials.json` into shared directories, tickets, or chat.
- On multi-user hosts, keep `$HOME` / `$GROK_HOME` private to your account.

### Re-authenticate

To switch accounts or resolve an authentication problem, run:

```bash
grok login
```

Running `grok login` starts the sign-in flow again, replacing your cached session. By default, it opens your browser and signs in through SpaceXAI OAuth at `auth.x.ai`. Pass a flag to select a different flow:

| Flag | Description |
|------|-------------|
| `--oauth` | Sign in through SpaceXAI OAuth at `auth.x.ai`. This is the default, so the flag is optional. |
| `--device-auth` (alias `--device-code`) | Sign in with the device-code flow for headless or remote environments. |

To sign out, run `grok logout`. It takes no flags and clears your cached credentials.

---

## API Key

For CI/CD, automation, or environments without browser access, use an API key from [console.x.ai](https://console.x.ai):

```bash
export XAI_API_KEY="xai-..."
grok
```

You can also store console / Business API keys in the **OS secret store** (keyring service `grok-build`). Interactive `grok login --api-key` uses a secure keyring path (ops are time-boxed so a hung Secret Service cannot block forever). On a TTY, after you accept the secret, Grok shows a short **stderr progress** line counting seconds up to the dual-backend budget (~6s = 2× the per-backend timeout) while the store RMW+write runs — so a hung Secret Service does not look like a freeze. Non-TTY / automation paths suppress the bar. On Linux, if the primary Secret Service store times out or errors, login **automatically** falls back to the kernel keyutils backend (no D-Bus unlock required). Only if **all** secure backends fail does login error out — it does **not** silently write the secret to a file as recovery. `GROK_CREDENTIALS_FORCE_FILE=1` is for tests/CI only, not a user recovery path. After a successful secure write, Grok may mirror under `$GROK_HOME/provider_credentials.json` (mode `0600`) for read resilience. When `XAI_API_KEY` is set, the environment value wins and is not written to the store.

### Store console keys (multi-add)

Never pass the secret as a command-line argument — it lands in shell history,
process lists, and some audit logs. Grok **refuses** argv secrets.

```bash
# Interactive: flag only, then type the key at the no-echo prompt
grok login --api-key

# Dual-auth status: session present? N store keys (fingerprints)? env wins?
# Never prints raw keys or tokens
grok login --list-api-keys
```

`grok login --list-api-keys` and human `grok doctor` / in-TUI **`/doctor`** both
show dual-auth discoverability (counts and fingerprints only): SuperGrok
session(s) with role labels when multi-principal, console key store count +
fingerprints, whether `XAI_API_KEY` env wins, preferred method, and whether
session+console failover is ready. Never prints raw keys or tokens.

Agents and scripts can also query **live sampling principal + spend meters**
without the TUI: `grok limits` (human) or `grok limits --json` (machine-readable,
no secrets). Same meter distinctions as in-session `/limits` — SuperGrok
included weekly %, SuperGrok dollar extras, console team prepaid when
Management is configured.

Prefer `XAI_API_KEY` for CI/automation (env wins; the store is not written when
env is set). Advanced: `grok login --api-key -` reads one line from **non-TTY**
process stdin only (not argv; a TTY stdin is refused — use bare `--api-key` for
the no-echo prompt). Do not put secrets in shell history. Stored multi-keys
become dual-auth failover candidates alongside SuperGrok OAuth.

**Console key order (stable):** keys are tried in this order after SuperGrok
session (or as primary when `preferred_method = "api_key"`):

1. `XAI_API_KEY` comma/newline list (left to right; env wins and is not written to the store)
2. Secret store multi-add order (`grok login --api-key` **appends**; first added is first tried)
3. `auth.json` `xai::api_key` (legacy single/multi blob; unique keys only)

To put a **Business / team** console key first: add it first with
`grok login --api-key`, or set `XAI_API_KEY=<business-key>` (optionally
`XAI_API_KEY=<business-key>,<other-key>`). There is no separate “preferred console
key” config yet.

### Two SuperGrok logins (personal + Business)

You can keep **two SuperGrok OAuth principals** at once (for example a personal
account and a Business / team SuperGrok session):

1. Sign in with the first account (`grok login` or the first-launch browser flow).
2. Sign in again as the second principal (OIDC as the other SuperGrok account).
   The first principal is **kept** in a multi-slot store; the second login does
   not wipe it.
3. Check with **`/doctor`** (dual-auth block) or **`grok login --list-api-keys`**.
   Both list SuperGrok principals with **role labels + fingerprints only** (no
   raw tokens or emails as secrets). Doctor also shows **billing poll health**
   for each SuperGrok role (last poll OK / auth failed / never this process)
   without secrets.

`/limits` and `grok limits` / `grok limits --json` stack SuperGrok sections when
two principals exist (for example `SuperGrok (personal)` and
`SuperGrok (business)`), with a live sampling line that names which principal is
active when known.

**Each principal is polled with its own JWT.** When one login is expired or
revoked (`no auth context`) and the other still works:

- Notes name the **role** (personal / business), a short **fingerprint**, and a
  re-login path (`grok login`). Do not rely on a bare 12-character id alone.
- Under **unified SuperGrok billing** (one shared free-period pool and one
  Extra Usage Credits pool for both logins), empty dual rows may still show the
  same free SuperGrok period % and the same SuperGrok dollar extras. That is
  **shared-pool fill**, labeled in human `/limits` and in JSON as
  `includedSource: "shared_pool_fill"` with `pollSucceeded: false` for the
  login that did not poll OK. A successful poll is `includedSource: "live_poll"`.
- Compact status / live free-period chrome uses the **active** SuperGrok
  principal only. If the active login's poll failed, free-period % is not
  painted healthy from the sibling alone (honest cold `...%` until re-login).

Meters that stay distinct even when dual SuperGrok share one free-period pool:

| Meter | What it is | What it is not |
|-------|------------|----------------|
| Free SuperGrok period allowance | Period quota usage % (included weekly/period) | SuperGrok $ top-ups; console prepaid; team invoice |
| SuperGrok Extra Usage Credits | Prepaid **dollar** top-ups on SuperGrok | Free period %; console team prepaid; team Grok Build class |
| Console team prepaid | Console API team prepaid wallet remaining | SuperGrok extras; free period; team postpaid invoice class |
| Team Grok Build / OAuth class $ | Team invoice settlement for OAuth / Grok Build | Free-period burn proof; SuperGrok extras as live driver; console key live |
| Console monthly spend limit | Separate console gate | Free SuperGrok period |

**Burn order (token economy):** free SuperGrok period first while used percent is
below 100 → then SuperGrok dollar credits (after-burner when free period is full)
→ then console API key. Status compact meter and `/limits` **Active:** line use
the same rule as **intent chrome** (for example compact `intent · 15%` while free
SuperGrok period has room). Team prepaid remaining and team Grok Build class
dollars can still climb under SuperGrok session without free SuperGrok period
moving; the prompt footer labels those as **Team settlement:** secondary chips,
not as a replacement for free SuperGrok period intent. That is not "on SuperGrok
dollar credits" and not a reason to paint `console · $N` while free SuperGrok
period has room.

**If status shows `console · $N` while free SuperGrok period still has room**
(for example `limits --json` shows SuperGrok live and included used under 100%):
that is a product chrome bug. Rebuild from a tree that includes the sticky-pin
headroom fix, then report if it still happens.

**Which SuperGrok login to re-auth:** open `/limits` or `grok doctor`, find the
role whose poll failed (auth), run `grok login` for that SuperGrok account.

Re-auth clears the active base session so you can sign in again; multi-slot
siblings stay until you log them out. Logout removes the active multi-slot (and
base); other SuperGrok principals remain. Console API keys are a separate path
for prepaid console spend (see multi-add above) and still work as failover
alongside SuperGrok sessions.

### SuperGrok session + console key (identity failover)

On first-party xAI models you may use **both** a consumer SuperGrok OAuth session (`grok login`) and a console / Business API key at once:

| Primary | Failover | When |
|---------|----------|------|
| Session (default) | Console key(s) from env, secret store, or `auth.json` (order above) | Both available; SuperGrok daily + Business when needed |
| Console key(s) | Remaining console keys, then session JWT last | `[auth] preferred_method = "api_key"` and both available |

By default (new or empty Grok home config), `[auth] auto_use_included_limits`
is **true**: the product prefers the **free SuperGrok allowance for the current
billing period** (personal and/or Business SuperGrok) before SuperGrok prepaid
top-up dollars and the console API key. When more than one SuperGrok login is
available, both free-period meters are considered and ranked among free-period
pools (sooner reset is a ranking heuristic). While **any** live SuperGrok login
still has free period allowance left (used percent below 100), console keys are
**omitted** from the sampling chain entirely (not primary and not silent
failover) so a mid-turn rate-limit or credit hop cannot burn console Grok Build
dollars while free SuperGrok period allowance remains. When one SuperGrok
login's free period is full, ranking fails over to another SuperGrok login that
still has free period left. When **every** SuperGrok free period is full but
SuperGrok **prepaid top-up dollars** still remain (session prepaid balance known
positive), the SuperGrok session stays primary and console keys are only
failover. When free period is full and top-up dollars are 0 or unknown, console
becomes primary. For a single principal, if the active base session was
refreshed but a multi-slot copy is still stale or marked out of allowance,
ranking uses the **live** SuperGrok JWT (including SuperGrok Heavy tier
sessions) rather than silently staying on the console API key.

This free-period-first setting is **not** a `preferred_method` value
(`preferred_method` stays `api_key` / `oidc` only, matching ordinary grok).
Hard pin: `[auth] preferred_method = "api_key"` stays console-primary even
when free SuperGrok period allowance remains (you chose console).

**Turn free-period-first off** (classic dual-auth: session primary + console
failover without free-period ranking) by setting in `$GROK_HOME/config.toml`:

```toml
[auth]
auto_use_included_limits = false
```

Existing homes that already set the flag (including explicit `false`) keep that
value. Only missing key / empty new config gets the default `true`.

### Free SuperGrok period debit unproven (flat poll)

The free-period-first **path** can be correct (SuperGrok session live, console
key not primary, Active: free SuperGrok period) while free SuperGrok period
used % never steps across multipoll and team Grok Build / OAuth settlement
dollars still climb. That is a **server debit** gap (not a client chrome bug).
The product does **not** invent free SuperGrok period used %.

**Default: new agent turns are allowed** so dogfood is not hard-stopped by the
server gap. `/limits`, multipoll, and doctor dual-auth status still show loud
honesty when free SuperGrok period limits stay flat (unproven debit) and team
settlement can move.

To **opt into a hard block** of new turns when free SuperGrok period still has
room and multipoll marks free SuperGrok period debit **unproven**
(`flatPollUnprovenDebit` on `grok limits --json`):

```toml
[auth]
allow_spend_when_free_period_debit_unproven = false
```

Or set env `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN=0` (falsy when set
blocks; truthy allows; unset uses config / default **true**). Console primary
pin (`preferred_method = "api_key"`) is not blocked by this guard.

When free SuperGrok period allowance is full, SuperGrok top-up dollars are gone
or unknown, and at least one console key is bound, sampling **prefers the first
live console key** (and `api.x.ai` when hosts are split). A **live SuperGrok
JWT stays on the recovery failover tail** so a console team credit or monthly
spending-limit 403 can hop back to free SuperGrok period (Retrying "out of
allowance"). Hard-expired SuperGrok is never recovery. When SuperGrok is also
dead, the terminal shows plain team admin English (add credits / raise monthly
spend limit), not an Internal error JSON envelope. Free SuperGrok period,
SuperGrok top-up dollars, and console team prepaid / monthly spend limit remain
**distinct meters**. When top-up dollars still remain under free-period-first
ranking (`auto_use_included_limits`), the SuperGrok session stays primary until
top-ups run out or a true credit/402 hop switches to console.

Mid-request hop uses the next configured identity when:

| Trigger | Behavior |
|---------|----------|
| **Credit / spending / SuperGrok Heavy usage limit** (HTTP 402, or credit-/usage-limit-worded 403/429/400 — including SuperGrok Heavy caps; not bare 403) | Switch identity immediately; remember the dead one is out of allowance (~1h memo under `$GROK_HOME/exhausted_credits/` + process cache; cleared on a later successful **console-key** request with that fingerprint — SuperGrok session success does not clear, so paid extras do not put SuperGrok back) |
| **Free SuperGrok period allowance at 100% used** (billing usage; dual-auth only) | When SuperGrok top-up dollars are 0 or unknown: mark SuperGrok out of allowance **before** the next request and prefer the console key (no HTTP 402 required). When free-period-first ranking is on (`auto_use_included_limits`) and top-up dollars remain positive: keep SuperGrok session; console only as failover. Memo clears when used percent drops below 100% (period reset) or that after-full path clears a prior mark |
| **Plain rate-limit 429** (no credit wording) | Switch identity immediately when another identity remains; temporary shared cooldown on the left key (not the allowance memo) so the primary can be tried again when cool |

Without a failover list, plain 429 still waits and retries on the same credential. OpenRouter and other BYOK hosts never receive the xAI session token. Enterprise `disable_api_key_auth` forces a single session identity and clears console-key failover.

See [Custom Models → Identity failover](11-custom-models.md#credit-failover-multi-account).

### Console team prepaid (Management API)

**Console team prepaid** is a separate meter from SuperGrok included weekly,
SuperGrok dollar extras (~$ extras on grok.com), and the inference console API
key (`XAI_API_KEY`). Product reads the **Management prepaid ledger** for your
console team via
[Management API](https://docs.x.ai/developers/rest-api-reference/management/billing)
(`GET …/billing/teams/{team_id}/prepaid/balance`) with a **Management key**
(different host: `management-api.x.ai`). An inference key alone cannot return
this balance. It is **not** SuperGrok $ extras and **not** a Business SuperGrok
OIDC login.

The browser dashboard on [console.x.ai](https://console.x.ai) uses a **web
session**. Grok cannot scrape that. The dashboard **Credits remaining** figure
may fold in other credit types (promo, default grants, or other ledger lines)
or lag the prepaid total. Product `console.teamPrepaidUsd` / footer **team
prepaid** is **only** `abs(total.val) / 100` from the prepaid balance endpoint
for the resolved team id — the **prepaid ledger**, not a claim that it always
matches every Credits remaining composite on the dashboard.

#### Setup

1. Create a **management key** in Console → Settings → **Management Keys**
   (permission: Management Keys Read). This is **not** an inference API key.
2. Store it securely (preferred):

```bash
grok login --management-key
# no-echo prompt; or non-TTY: printf '%s\n' "$KEY" | grok login --management-key -
```

   Prefer env for CI/headless: `export XAI_MANAGEMENT_API_KEY=...` (never put
   secrets on argv).

3. **Team id** (path param on Management routes):
   - **Auto-discovered** from `GET /auth/management-keys/validation` only when
     the stored secret is a **valid Management key** (login prints the id).
   - If login says the API **rejected** the key (HTTP 401), you pasted an
     **inference API key** or a revoked secret — create a Management key under
     Console → Settings → **Management Keys** and re-run login. That is not a
     missing team-id problem.
   - Always safe to **pin** the console team UUID (team settings / console URL):

```toml
# ~/.grok/config.toml
[endpoints]
# Optional if key is in secret store or XAI_MANAGEMENT_API_KEY.
# management_api_key = "xai-..."
management_team_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
```

```bash
export XAI_MANAGEMENT_TEAM_ID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
```

Do **not** assume SuperGrok OIDC `team_id` equals the console Management team
id.

Management key secret store uses keyring service `grok-build` under the URL
key `https://management-api.x.ai` — distinct from the inference console slot
(`https://api.x.ai/v1`). Never pass management keys as command-line arguments.

#### Surfaces

When the Management key is available (and team id is pinned or discovered),
billing refresh (session start, turn end, `/usage`, `grok limits`) fetches
the prepaid ledger **regardless of whether console or SuperGrok is live** and
fills:

- Footer when **console** is live: `Console key · team prepaid: $N` (optional
  `team Grok Build class: $N` when known). Under console live, team prepaid is
  the live console pool.
- Footer when **SuperGrok** is live and team prepaid and/or Grok Build class is
  known: free SuperGrok period remaining / SuperGrok dollar credits when those
  warn, **plus** labeled settlement such as
  `Team settlement: prepaid $N · Grok Build class $M` (or a standalone Team
  settlement chip when SuperGrok alone would be quiet). Team settlement never
  re-labels live sampling as console and never replaces compact
  `intent · N%` while free SuperGrok period has room.
- Status compact meter (upper-right): free SuperGrok period paints as
  `intent · N%` (or SuperGrok extras `$` after free SuperGrok period is full).
  Team prepaid dollars do not paint there while free SuperGrok period drives.
- `/limits` / `grok limits`: `Team prepaid remaining: $N` under **Console API**
  even when `console.isLive` is false (SuperGrok serving). Console team
  **prepaid ledger** is never labeled SuperGrok dollar credits; it may differ
  from dashboard Credits remaining when the UI includes more than prepaid.
- `grok limits --json`: `console.teamPrepaidUsd` (or `console.teamPrepaidGap`);
  `activeDriver` stays free SuperGrok period / SuperGrok extras / console key
  (intent only, not team settlement).

Honest gap copy is **distinct** by what is missing (no invented balance):

| State | Footer / `/limits` Balance line |
|-------|----------------------------------|
| No management key | **no management key** (always on `/limits` Balance; SuperGrok-only footer stays SuperGrok-focused) |
| Key set; team id still unknown after fetch | **no management team id** |
| Key set; fetch in flight / cold | **loading team prepaid...** (SuperGrok live footer: `Team settlement: loading team prepaid...`) |
| Key + team known; fetch done but no balance | **team prepaid unavailable** (SuperGrok live footer under Team settlement) |
| Balance known | SuperGrok live: **Team settlement: prepaid $N**; console live: **team prepaid: $N**; `/limits`: **Team prepaid remaining: $N** |

`grok limits` also prints a short **Notes** hint when the management key or
team id is missing (how to configure). SuperGrok $ extras stay SuperGrok-only.

When postpaid preview is available, `/limits` and `grok limits` also show:

- **Team default credits** (dashboard allotment from postpaid `defaultCredits`,
  often about $1500 on the wire) as its **own** labeled line. That number is
  **not** the console team prepaid wallet, **not** free SuperGrok period
  allowance, and **not** SuperGrok prepaid top-up dollars. JSON:
  `console.teamDefaultCreditsUsd`.
- **Team usage series** (Management `POST …/billing/teams/{team_id}/usage` with
  `analyticsRequest`): a short day-window summary with OAuth / Grok Build class
  vs API-key class totals (and top description rows). Not a GET invent. JSON:
  `console.teamUsageSeriesOauthClassUsd` / `teamUsageSeriesApiClassUsd` plus
  window start/end. Refreshes with other Management meters on TUI `/limits`,
  background billing (soft ~60s cache), and `grok limits` — not a rare CLI-only
  path. Full browser-style chart UI is still not a product goal; text totals are.

#### Three surfaces that look similar (keep distinct)

| Surface | What it is | Product |
|---------|------------|---------|
| **Platforms → Grok Business → licenses Usage** (browser: messages / conversations / active users) | Seat/license product charts on console.x.ai | **Not wired.** No client. Zeros there do not prove SuperGrok or team Management are idle. |
| **Team API Usage $** (browser Usage / Management postpaid + series) | Team OAuth vs API class **dollars** | `/limits` postpaid + usage series when management key works |
| **TUI SuperGrok + Management meters** | SuperGrok included % / extras; team prepaid / postpaid / series | Status, footer, `/limits`, `grok limits` |

Enterprise `GROK_DEPLOYMENT_KEY` is a different surface (managed config /
attribution), not a substitute for these meters.

#### If Grok Business license Usage is all zeros

**Expected for CLI SuperGrok dogfood.** Platforms → **Grok Business** →
**Usage** (`.../grok-business/usage`) counts license seat activity (messages,
conversations, active users). grok-oss SuperGrok traffic uses the session /
cli-chat-proxy path; it does **not** drive those seat counters. There is no
public Management API for license message charts, and this product does not
scrape console HTML or invent counts.

**Where real burn shows:**

1. **Product:** `grok-oss limits` or TUI `/limits` — team postpaid OAuth
   (**Grok Build class**) and usage series when a management key is set;
   SuperGrok included % / extras; team prepaid remaining (separate wallet).
2. **Browser:** team **Usage** spend charts (`console.x.ai` team `.../usage`,
   not the licenses page) — often mostly **Grok Build**.

Zeros on the licenses page do **not** mean dogfood is idle. See also
`grok doctor` (dogfood burn proof block) and the always-on `/limits` note.

#### Prefer free SuperGrok period limits after a period reset

When the free SuperGrok period has reset and you want session dogfood on free
SuperGrok period limits (not SuperGrok dollar credits or console burn first):

```toml
# ~/.grok/config.toml
[auth]
preferred_method = "oidc"            # SuperGrok session primary
auto_use_included_limits = true      # free SuperGrok period limits before credits / console
```

That pairing does **not** fill Grok Business **license** charts. It only ranks
credentials so free SuperGrok period limits are preferred while they still have
room.

#### Token economy proof checklist

Use this short checklist before and after SuperGrok dogfood when you need
**evidence** that the client path is right (P1) and whether free SuperGrok
period limits moved (P2). Meters stay distinct: free SuperGrok period limits ≠
SuperGrok dollar credits ≠ console team prepaid / console API credits.

1. **Hermetic path suite (no network):** run `just check-limits-first-path`.
   This is the unit-test contract for limits-first ranking and the pure
   `limits --json` path checker. It does not measure live free SuperGrok period
   debit.
2. **Live path gate (one sample):** rebuild or install, then run
   `just check-limits-first-live`. That runs `grok-oss limits --json` once and
   applies the same pure path checker. Expect SuperGrok session primary and
   `console.isLive=false` while free SuperGrok period used is still below 100%
   under default free-period-first config.
3. **Multipoll harness (before and after SuperGrok dogfood):**
   `just limits-multipoll` or `grok-oss limits multipoll` (default **2** samples
   and **30s** sleep between sample ends so the flat-poll window can fire).
   Writes `samples.jsonl`, `fields.jsonl`, and `summary.json` under
   `.agents/reports/limits-multipoll-<utc>/` (override with `--out-dir` or
   `LIMITS_MULTIPOLL_OUT_DIR`). Capture once before a dogfood window and once
   after so you can compare free SuperGrok period used % and team settlement
   dollars.
4. **How to read P1 vs P2:**
   - **P1 path:** process exit and `pathOk` / `pathStatus` in the multipoll
     summary. Path **fail** means console was live primary while free SuperGrok
     period limits still had room (Design A broken). Path **OK** means the
     client stayed on SuperGrok when free SuperGrok period limits still had room
     (or the checker skipped for config reasons).
   - **P2 free SuperGrok period series:** `freePeriodSeries` is `flat`,
     `stepped`, or `insufficient`. **Flat at 6% (or any fixed %) is not a
     client path failure** and is **not** proof that the product left free
     SuperGrok period limits. It is measurement that free SuperGrok period used
     % did not step across the multipoll window (server debit still unproven).
     Stepped means free SuperGrok period used % moved between samples. Exit code
     is **non-zero only on path failure**; flat free SuperGrok period limits
     always keep exit 0 when path is OK.

Spend order while free SuperGrok period limits still have room: free SuperGrok
period limits first, then SuperGrok dollar credits, then console team prepaid /
console API credits. Stay on SuperGrok session; do not make the console API key
primary. Never invent free SuperGrok period used % on the client.

---

## OIDC (Customer SSO)

Authenticate developers through your own Identity Provider (IdP) -- such as Okta, Azure AD, or Auth0 -- instead of grok.com.

### 1. Register a public client in your IdP

- Grant type: Authorization Code with PKCE (Proof Key for Code Exchange)
- Redirect URI: `http://127.0.0.1/callback` -- a loopback address. Grok binds a random port at sign-in time, and most IdPs treat the loopback redirect as port-agnostic per [RFC 8252](https://tools.ietf.org/html/rfc8252).
- No client secret. PKCE replaces it.

### 2. Configure the CLI

Via config file:

```toml
# ~/.grok/config.toml
[grok_com_config.oidc]
issuer = "https://acme.okta.com"
client_id = "0oa1b2c3d4e5f6g7h8i9"
```

Or via environment variables:

```bash
export GROK_OIDC_ISSUER="https://acme.okta.com"
export GROK_OIDC_CLIENT_ID="0oa1b2c3d4e5f6g7h8i9"
```

You can also override the API endpoint to point at your own proxy:

```bash
export GROK_CLI_CHAT_PROXY_BASE_URL="https://grok-proxy.acme.com/v1"
```

### 3. Run `grok`

The CLI discovers endpoints via `{issuer}/.well-known/openid-configuration`, opens the IdP login page, and stores tokens in `~/.grok/auth.json`. Tokens auto-refresh silently via the stored `refresh_token`.

### Optional fields

| Field | Default | Notes |
|-------|---------|-------|
| `scopes` | `["openid", "profile", "email", "offline_access", "api:access"]` | `offline_access` enables silent token refresh |
| `audience` | None | Required by some IdPs (e.g., Auth0) |

---

## External Auth Provider

When browser-based login isn't possible -- for example, on sandboxed VMs, CI runners, or air-gapped networks -- delegate authentication to an external binary or script.

### How It Works

```
+--------------+     sh -c     +------------------------+
|     Grok     |-------------->|  your auth binary      |
|              |               |                        |
|  reads       |<-- stdout ----|  prints token          |
|  auth.json   |               |                        |
|              |   (stderr)    |  prints status/URLs    |--> surfaced to user
+--------------+               +------------------------+
```

1. Grok runs your command via `sh -c "<command>"`
2. Your binary runs whatever auth flow it needs (SSO, device code, certificate exchange)
3. **stderr** carries human-readable output, such as login URLs and status messages. Grok reads stderr and surfaces it to the user; in the TUI, it turns the first `https://` URL into a clickable sign-in link.
4. **stdout** is captured by Grok and saved as the access token
5. Exit 0 = success; exit non-zero = Grok falls back to interactive login

### The stdout / stderr Contract

| Stream | What to print | Who sees it |
|--------|---------------|-------------|
| **stdout** | The token -- nothing else | Grok (parsed and stored in auth.json) |
| **stderr** | Login URLs, status messages, errors | The user (Grok reads stderr and shows the sign-in URL as a clickable link in the TUI) |

**Do not print anything to stdout except the token.** No progress messages, no debug output. Grok reads stdout, trims surrounding whitespace, and parses the result as a token.

### stdout Token Format

**Bare string** -- just the raw token:

```
eyJhbGciOiJSUzI1NiIs...
```

**JSON** -- with optional refresh token, expiry, and issuer:

```json
{"access_token": "eyJhbGciOi...", "refresh_token": "ref-tok", "expires_in": 3600, "issuer": "https://idp.example.com"}
```

Use JSON if your tokens expire and you want Grok to automatically re-run the binary before expiry.

JSON fields:

| Field | Required | Meaning |
|-------|----------|---------|
| `access_token` | yes | Bearer token Grok sends to the xAI API |
| `refresh_token` | no | Stored for reference. Grok refreshes by re-running your binary, not with an OAuth refresh grant |
| `expires_in` | no | Token lifetime in seconds; enables proactive refresh before expiry |
| `issuer` | no | Identifies the token's issuer |

### Configuration

Via config file:

```toml
# ~/.grok/config.toml
[auth]
auth_provider_command = "/usr/local/bin/my-auth-provider"
auth_provider_label = "Acme Corp"   # optional -- customizes the TUI login button
auth_token_ttl = 3600               # optional -- token lifetime in seconds
```

Or via environment variables:

```bash
export GROK_AUTH_PROVIDER_COMMAND="/usr/local/bin/my-auth-provider"
export GROK_AUTH_PROVIDER_LABEL="Acme Corp"
export GROK_AUTH_TOKEN_TTL=3600
```

### Token Refresh

When Grok needs to refresh an expired token, it re-runs your binary with `GROK_AUTH_EXPIRED=1` set in the environment. Each run fully replaces the stored credential, so emit the same JSON fields (such as `issuer`) on every invocation, including refreshes. Your binary can use this to take a faster silent-refresh path:

```bash
#!/bin/sh
if [ "$GROK_AUTH_EXPIRED" = "1" ]; then
    echo "Refreshing token..." >&2
    TOKEN=$(my-company-auth --refresh --silent)
else
    echo "Authenticating via Acme Corp SSO..." >&2
    TOKEN=$(my-company-auth --login --interactive)
fi

if [ -z "$TOKEN" ]; then
    echo "Authentication failed" >&2
    exit 1
fi

echo "{\"access_token\": \"$TOKEN\", \"expires_in\": 3600}"
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `GROK_AUTH_PROVIDER_COMMAND` | Path to your auth binary |
| `GROK_AUTH_PROVIDER_LABEL` | Display name on the TUI login screen (e.g., "Acme Corp") |
| `GROK_AUTH_TOKEN_TTL` | Token lifetime in seconds (for bare-string tokens without `expires_in`) |
| `GROK_AUTH_EXPIRED` | Set to `1` by Grok when re-running the binary for token refresh |
| `GROK_AUTH_EARLY_INVALIDATION_SECS` | Seconds before expiry to proactively refresh (default: 300) |

---

## Device Code Flow

For headless environments (SSH sessions, Docker containers, remote VMs) where no browser is available locally:

```bash
grok login --device-auth    # or: grok login --device-code
```

This prints a URL and code to the terminal. Open the URL on any device, enter the code, and complete authentication. Grok polls until the login is confirmed.

You can also implement the device-code flow through an [External Auth Provider](#external-auth-provider) for full control.

---

## Automatic Credential Refresh

Grok automatically refreshes expired credentials:

- **Before expiry:** If your auth provider returned `expires_in` (JSON output) or you set `auth_token_ttl`, Grok re-runs the auth binary ~5 minutes before expiry.
- **On auth error:** If the server returns 401 Unauthorized, Grok refreshes the credentials and retries the request.
- **OIDC:** If a `refresh_token` is available, Grok silently refreshes via your IdP without re-opening the browser.

Tune the refresh buffer:

```bash
# Refresh 5 minutes before expiry (default)
export GROK_AUTH_EARLY_INVALIDATION_SECS=300

# Disable the proactive buffer: refresh at expiry or on a 401 (set to 0)
export GROK_AUTH_EARLY_INVALIDATION_SECS=0
```

---

## Hot Reload

Grok picks up changes to `~/.grok/auth.json` automatically. If you update credentials externally (for example, with a script that writes new tokens), Grok uses the new credentials on the next API call without a restart.

---

## Auth Precedence

Grok resolves credentials for each request in this order, highest to lowest:

1. **Per-model `api_key` or `env_key`** -- set under `[model.<name>]` in `config.toml`. Wins whenever present.
2. **Active session token** -- obtained through browser, OIDC/OAuth2, or external-provider login and stored in `~/.grok/auth.json`.
3. **`XAI_API_KEY`** -- fallback when no session token is active.

When more than one login flow is configured, Grok populates the session token from the first available source, highest to lowest:

1. **External auth provider** (`auth_provider_command`)
2. **Enterprise OIDC** -- when OIDC is configured, through `[grok_com_config.oidc]` in `config.toml` or the `GROK_OIDC_ISSUER` and `GROK_OIDC_CLIENT_ID` environment variables
3. **SpaceXAI OAuth2 browser login** -- the default

During a session, the active method handles all mid-session refreshes.

---

## Related settings

`/privacy` does not change these config knobs:

| Setting | How to set it |
|---------|---------------|
| `[features] telemetry` | `config.toml` or `GROK_TELEMETRY_ENABLED` |
| `[telemetry] trace_upload` | `config.toml` or `GROK_TELEMETRY_TRACE_UPLOAD` |
| External OpenTelemetry | `GROK_EXTERNAL_OTEL` / `[telemetry] otel_*`. See [Monitoring Usage](24-monitoring-usage.md). |

On team accounts, only a team admin can toggle privacy with `/privacy`.
Team admins can also enable or disable Zero Data Retention (ZDR) for their team.
See [How to enable ZDR](https://docs.x.ai/developers/faq/security#how-to-enable-zdr).
When ZDR is on, `/privacy` cannot change coding-data sharing.

See [Monitoring Usage](24-monitoring-usage.md#related-settings) and [Configuration](05-configuration.md#telemetry).

---

## Troubleshooting

### Debug logging

Set `RUST_LOG` to control the verbosity of the file log and headless stderr output. (The TUI's on-screen tracing pane uses a fixed filter and ignores `RUST_LOG`.) In the TUI, file logging defaults to `DEBUG`; in headless mode (`-p`), `RUST_LOG` defaults to `off` so only the answer is printed — set `RUST_LOG=error` (or broader) to see logs on stderr.

In the TUI, set `GROK_LOG_FILE` to an absolute path to write logs to that file:

```bash
GROK_LOG_FILE=/tmp/grok.log RUST_LOG=debug grok
tail -f /tmp/grok.log
```

`GROK_LOG_FILE` is treated as a literal file path. A relative value such as `1` writes a file named `1` in the current directory.

In headless mode, logs go to stderr. Redirect them to a file:

```bash
RUST_LOG=debug grok -p "hello" 2> /tmp/grok.log
```

### Common log messages

| Log message | What it means |
|-------------|---------------|
| `auth: running external auth provider` | Grok is running your binary |
| `auth: external auth provider returned fresh token` | Grok parsed and stored the token |
| `auth: external auth provider failed` | Binary exited non-zero or stdout was empty |
| `auth: external auth provider timed out (likely needs interactive auth), killing` | Binary did not exit before the timeout and was killed |
| `auth: failed to start external auth provider` | Command could not be spawned (binary not found) |

### Common fixes

- **"Authentication failed"** -- Run `grok logout` to clear cached credentials, then `grok login` to sign in again.
- **Token expires too quickly** -- Set `auth_token_ttl` or return `expires_in` in your auth provider's JSON output.
- **OIDC redirect fails** -- Ensure your IdP allows loopback redirect URIs (`http://127.0.0.1/callback`).
- **External auth provider not found** -- Check that the `auth_provider_command` path is correct and the binary is executable.
