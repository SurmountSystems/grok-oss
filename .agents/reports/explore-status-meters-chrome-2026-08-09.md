# Explore: status bar billing / limits chrome (why A ≠ B)

**Date:** 2026-08-09
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Mode:** read-only map of product code (no product edits)

## Operator screenshots (plain reading)

| | Image A (grok-build) | Image B (bitmagi) |
|--|----------------------|-------------------|
| **Upper-right green** | `15% · 66% behind linear burn` | May match (free SuperGrok period style) |
| **Prompt footer** | `Team prepaid: $340 · team Grok Build class: $1162.92 · Grok 4.5 (high) · always-approve` | `Grok 4.5 (high) · always-approve` only |
| **Unclear** | Which meter is **paying** vs **intent** vs **settlement side-channel** | Same free-period chrome; no team $ chips |

These are **two different chrome surfaces**, not one unified “paying meter” label.

---

## 1. Where chrome is built (file:line)

### A. Upper-right compact meter (status bar `"credits"` segment)

| Piece | Path |
|-------|------|
| Wire-up paint | `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` ~1455–1540 |
| SuperGrok line + pacing | `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` `credit_bar_line_for_session` ~1200–1257 |
| Design A pure string | same file `compact_meter_text_for_live_identity*` ~1286–1367 |
| Sticky identity for paint | `status_sampling_identity_for_compact_meter` ~308–325; used in render ~1475–1490 |
| Linear-burn chip | `CreditBalance::pacing_chip` ~386–397 → `token_economy/period_pacing.rs` `compact_label` ~26–36 |
| Config gate for pacing | `[token_economy] show_period_pacing` via `period_pacing` ~367–374 |

**Design A (active meter only):**

| Live identity | Compact string |
|---------------|----------------|
| Console live | `console · $N` or `console · {gap}` |
| SuperGrok + free period has room (`< 100%`) | `N%` (+ optional ` · X% behind/ahead of linear burn`) |
| SuperGrok + free period full + SuperGrok $ extras > 0 | `SuperGrok extras · $N` (no pacing) |
| SuperGrok + full + no extras | `100%` |
| SuperGrok cold / active poll auth-failed | `...%` |
| Gateway `chat_kind` | **hidden** (no coding-credits meter) |

**Not gated** by consumer `billing_surface_visible` / `usage_visible`. Team dual-auth dogfood keeps free-period `%` even when team principal hides consumer `/usage manage` (tests ~5198–5217, 5310–5326).

**Does not paint** team prepaid `$` or team Grok Build class on this surface while SuperGrok free period has room (explicit test: free period 6% + team prepaid $340 → still `6%`, not `console · $340` ~5380–5409).

### B. Prompt footer usage strip (team prepaid / Grok Build class / SuperGrok extras warning)

| Piece | Path |
|-------|------|
| Wire-up | `render.rs` ~2573–2641 |
| Policy | `credit_bar.rs` `usage_warning_for_session_with_identity_principal_gap_and_postpaid` ~943–1008 |
| SuperGrok-only warning | `supergrok_session_usage_warning` ~1025–1104 |
| Team merge | `merge_supergrok_warning_with_team_meters` ~1112–1167 |
| Grok Build chip | `team_grok_build_class_footer_chip` ~1014–1020 |
| Gap phrases | `ConsoleTeamPrepaidGap::as_display_str` ~117–125 |

Footer also always includes **model** (`Grok 4.5 (high)`) and **permission mode** (`always-approve`) via `PromptInfo` (~2642–2652). Those are independent of billing meters.

### C. Intent chrome: `activeDriver` / Active:

| Piece | Path |
|-------|------|
| Enum + wire labels | `credit_bar.rs` `ActiveSpendDriver` ~243–273 (`supergrok_free_period` \| `supergrok_extras` \| `console_key`) |
| Resolve | `active_spend_driver` ~280–296 |
| `/limits` human line | `limits_snapshot.rs` `active_driver_line_for_snapshot` ~629–653 |
| JSON | `limits_cmd.rs` builds `activeDriver` from same Design A order |
| Honesty (not settlement proof) | `limits_honesty.rs` `NOTE_ACTIVE_DRIVER_IS_INTENT_NOT_SETTLEMENT` ~53–67 |

**`activeDriver` never names team prepaid or Grok Build class as the “active” pay path.** Docs and notes say it is **client spend-order intent**, not “who settles the bill.”

### D. Settlement meters (data path, not “who is paying” chip)

| Meter | Source | Cache |
|-------|--------|--------|
| Free SuperGrok period used % | SuperGrok billing extension (`x.ai/billing`) → `CreditBalance.usage_pct` | Agent field |
| SuperGrok dollar credits (extras) | Same billing `prepaid_balance_cents` | Agent field |
| Console team prepaid remaining | Management `GET …/prepaid/balance` | Process cache ≤60s (`CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS`) |
| Team postpaid OAuth / Grok Build class $ | Management postpaid preview | Same process cache family |
| Fill path | `Effect::FetchBilling` always joins SuperGrok + management prepaid + postpaid + usage series (`effects/mod.rs` ~4231–4248) | Per-process |

---

## 2. Show / hide matrix

### Upper-right compact (status `"credits"`)

| Condition | Show | Hide / omit |
|-----------|------|-------------|
| `chat_kind` (gateway chat) | — | entire meter |
| SuperGrok live, included known, room | `N%` + optional pacing | team $ |
| SuperGrok live, full + extras | `SuperGrok extras · $N` | free-period % as sole driver |
| Console live (tracked or sticky memo after free period full) | `console · $N` / gap | SuperGrok `N%` |
| Free period headroom + sticky exhaust memo | **forces SuperGrok** (blocks false `console · $`) | console paint |
| `billing_surface_visible == false` | **still shows** meter | does not hide |
| Project cwd | only shortens cwd path elsewhere | **no meter gate** |

### Footer usage_warning (team prepaid / Grok Build / SuperGrok warnings)

Hard entry gate (`credit_bar.rs` ~956–958):

```text
if gateway_chat || !usage_visible { return None; }
```

`usage_visible` / `billing_surface_visible` set in `app_view.rs` ~1383:

```text
usage_visible = team_name.is_none() && !is_api_key_auth
```

| Condition | Footer billing chips |
|-----------|----------------------|
| `gateway_chat` | **none** |
| `team_name` set (enterprise AuthMeta) | **none** (consumer surface off) |
| API key primary auth | **none** |
| Personal SuperGrok + `usage_visible` | open |
| OpenRouter model active | `OpenRouter credits left: $N` (replaces xAI path) |
| Console live | `Console key · team prepaid: $N` (or gap) · optional `team Grok Build class: $N` |
| SuperGrok live + free period &lt; 90% + no extras drawdown | **no SuperGrok % warning alone** |
| SuperGrok live + team prepaid cents known | **`Team prepaid: $N`** even when SuperGrok warning is quiet |
| SuperGrok live + postpaid OAuth class cents &gt; 0 | **`team Grok Build class: $N`** |
| SuperGrok live + free period ≥ 100% + SuperGrok extras + autotopup gates | `SuperGrok extras left: $N` |
| SuperGrok live + free period &gt; 90% (no extras path) | `Weekly/Monthly limit left: R%` |
| Management key missing | **omit** prepaid gap chip under SuperGrok (`MissingManagementKey` → no team append) |
| Postpaid class 0 or unknown | omit Grok Build chip |

So footer team $ chips require **all** of:

1. Not chat gateway
2. `billing_surface_visible` true (personal SuperGrok, not team AuthMeta, not API-key primary)
3. Management process cache (or agent field) has prepaid cents and/or postpaid OAuth class cents
4. Binary includes SuperGrok-live team merge (shipped 2026-08-04 prepaid, 2026-08-07 Grok Build class)

### Vocabulary (product pins)

| Plain name | What it is | Chrome home today |
|------------|------------|-------------------|
| **Free SuperGrok period limits** | Included period used % (not dollars) | Upper-right Design A; `activeDriver=supergrok_free_period` while room |
| **SuperGrok dollar credits** | Personal top-up / extras `$` | Compact when period full; footer “SuperGrok extras left” when gated |
| **Console team prepaid** | Team Billing Credits remaining (Management) | Footer chip; compact only when **console live** |
| **Team postpaid OAuth / Grok Build class** | Period settlement class $ on team invoice | Footer chip + `/limits`; never Design A compact while free period drives |
| **Desired spend order** | Free SuperGrok period → SuperGrok $ credits → console team prepaid / API | Rank / hop / Design A; **not** settlement proof |

---

## 3. Upper-right green free SuperGrok period + “behind linear burn”

**Source of `15%`:** `CreditBalance.usage_pct` floored/formatted as `{pct:.0}%` under SuperGrok live with free period room (`compact_meter_text…` ~1364–1366; paint via `credit_bar_line_for_session`).

**Source of `66% behind linear burn`:**
`period_pacing.rs` compares free SuperGrok period used % to time share of billing period (`compute_period_pacing`). Chip text: `"{d}% behind linear burn"` when used % is **below** linear expectation (using slower than calendar share).

**When shown:** SuperGrok free-period compact path **and** `[token_economy] show_period_pacing` true **and** period end + derivable start (weekly/monthly type) **and** chip length ≤ 28 (`credit_bar.rs` ~1232–1234). Not shown on SuperGrok-extras `$` compact path. Never means “team is not paying.”

Color: green (`theme.accent_success`) when free-period used % &lt; 80 (`credit_bar_line_for_session` ~1246–1253).

---

## 4. `activeDriver` / live sampling vs which chrome dominates

| Concept | Role |
|---------|------|
| `SamplingIdentityKind` | Live sampling principal (SuperGrok session vs console key). Drives Design A branch and footer console vs SuperGrok branch. |
| `status_sampling_identity_for_compact_meter` | Free SuperGrok period headroom **wins paint** over sticky exhaust memo (limits-before-credits). |
| `meter_sampling_identity` | Older/helper path: sticky memo can pin console when free period full/unknown. |
| `ActiveSpendDriver` / `activeDriver` | Same Design A order as compact intent: free period → SuperGrok extras → console. **Intent only.** |
| Team prepaid / Grok Build class | **Settlement / Management side-channel.** Can move under SuperGrok session with free period flat; product notes that explicitly (`limits_honesty.rs`, residual settlement gap). |

**Implication for Image A:** upper-right says free SuperGrok period is the **intent driver** (15% used). Footer team prepaid + Grok Build class say **team wallets still exist and can settle** SuperGrok-session traffic. Product does **not** currently promote one line that says “paying: team prepaid / Grok Build class.” That is open residual (`RESIDUAL.md` settlement pay-path; soft `payingMeter`).

---

## 5. Why A ≠ B (session / project / process)

**Not project-cwd gated.** Meter policy does not branch on repo path. Cwd only affects path shortening in the status bar.

Likely causes ranked by code evidence:

### 1. Footer hard gate: `billing_surface_visible` / AuthMeta (strong)

If bitmagi session AuthMeta has **`team_name`** or **API-key primary**, entire footer usage_warning is `None` while compact free-period `%` still paints.
Image A with team prepaid chips **requires** `usage_visible == true` (personal SuperGrok principal).
If both sessions share the same personal SuperGrok login, this alone does **not** explain A vs B; if AuthMeta differs between processes/accounts, it does.

### 2. Management process cache cold / missing in one process (strong for same auth)

Team prepaid and Grok Build class come from **per-process** Management cache filled by `FetchBilling`.
If bitmagi:

- has no management key in that process’s config view, or
- never completed management fetch yet, or
- postpaid class is 0 / uncached,

then SuperGrok at 15% produces **no** SuperGrok % warning and **no** team chips → footer is only model + mode (Image B).
Grok-build session that already warmed cache shows Image A.

### 3. Binary / install skew (strong when projects use different binaries)

Grok Build class footer chip + SuperGrok-live team merge are recent (2026-08-04 / 2026-08-07).
Dev tree `grok-build` vs older installed `grok-oss` on bitmagi would match A has chips / B does not, while free-period compact still works on both.

### 4. Timing of first billing poll (medium)

Silent `FetchBilling` on turn end + periodic poll (`event_loop.rs` ~2334–2347). SuperGrok `%` can warm before management postpaid class lands (or management fails silently while SuperGrok succeeds). Screenshot B mid-warm → no team line.

### 5. Not: “project disables team meters”

No code path hides team meters based on cwd / project name. Dual-auth config is `$GROK_HOME` + process, not workspace.

---

## 6. Plan-relevant design options (always clear which meter is paying + consistent chrome)

Residual already tracks the gap: honesty note shipped; machine **`payingMeter`** and settlement-first compact **not** shipped (`RESIDUAL.md` ~506–534).

| Option | What it would do | Tradeoff |
|--------|------------------|----------|
| **A. Dual-line always-on chrome** | Compact: keep Design A free-period / extras / console intent. Footer (or second status chip): always show settlement side-channel when known (`Team prepaid $N · Grok Build class $M`) **even when** `billing_surface_visible` is false for team AuthMeta. | Fixes team-principal hide of the very meters team dogfood needs; still does not claim “paying.” |
| **B. Primary “Paying (intent)” + “Settlement (observed)” labels** | Status: `Intent: free SuperGrok period 15%`. Footer or `/limits` top: `Settlement tracked: team prepaid $340 · Grok Build class $1163` with note that settlement is not free-period debit proof. | Matches existing honesty constants; reduces “unclear which meter is actively paying.” |
| **C. Machine `payingMeter` from deltas** | Only claim pay path after measured prepaid remaining drop and/or Grok Build class rise across polls (never invent free SuperGrok period debit). | Highest honesty; needs multipoll history; soft residual. |
| **D. Unify gate** | Decouple team Management chips from consumer `usage_visible` (team_name / api_key). Keep consumer `/usage manage` hidden for teams; keep settlement chips visible. | Direct fix for “team principal → no footer team $.” |
| **E. Consistent cold chrome** | When management key configured but cold: show `loading team prepaid...` / honest gap on SuperGrok live footer (today MissingManagementKey suppresses gap under SuperGrok; Loading/Unavailable can show). | Makes A vs B timing less confusing; avoid inventing $0. |
| **F. Do not mash** | Keep free SuperGrok period % ≠ SuperGrok dollar credits ≠ console team prepaid ≠ Grok Build class as separate named chips forever. | Already law; any “one number” UI must label intent vs settlement. |

**Recommended plan spine for consistency (no code claimed done here):**

1. **Decouple settlement chips from `billing_surface_visible`** (option D) so every SuperGrok-session TUI on the same machine with management configured paints the same team lines.
2. **Label intent vs settlement** in chrome copy (option B) so Image A is readable: free period is intent; team $ are settlement trackers.
3. Leave Design A compact free-period-first unless operator explicitly wants settlement-primary compact (residual notes that would fight free-period-first on purpose).
4. Optional later: `payingMeter` from deltas (option C).

---

## 7. Quick map for implementers

```text
Status bar upper-right
  render.rs paint
    → status_sampling_identity_for_compact_meter
    → credit_bar_line_for_session / compact_meter_text…
    → optional period_pacing chip
  Gates: !chat_kind only (not billing_surface)

Prompt footer strip
  render.rs usage_warning_…
    → gated by billing_surface_visible + !chat_kind
    → SuperGrok extras / high % OR team prepaid OR Grok Build class
  Model + always-approve always separate

activeDriver / Active:
  intent only (free period | extras | console key)
  never team prepaid / Grok Build class
```

---

## 8. Related artifacts

- `RESIDUAL.md` settlement pay-path + half-B team meters
- `FORK.md` free SuperGrok period always before credits; billing meters half A/B
- `.agents/reports/bug-limits-chrome-when-on-credits-2026-08-07.md` (Design A)
- `.agents/reports/verify-compact-status-chrome-2026-08-07.md`
- `.agents/reports/impl-settlement-pay-path-tracking-gap-2026-08-09.md`
- `.agents/reports/impl-supergrok-live-team-usage-2026-08-04.md`
- `.agents/reports/impl-grok-business-license-zeros-vs-team-usage-2026-08-07.md`

**Bottom line:** Image A is SuperGrok **intent** chrome (15% + linear burn) plus **settlement side-channel** footer chips (team prepaid + Grok Build class). Image B is the same intent compact path without those footer chips, most often because `billing_surface_visible` is off, management cache/binary lacks the chips, or both—not because bitmagi is a different “project billing mode.” Neither surface currently answers “which meter is paying” as a single authoritative line.
