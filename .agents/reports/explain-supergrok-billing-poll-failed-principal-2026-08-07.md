# SuperGrok billing poll failed principal (plain English)

Date: 2026-08-07
Scope: read-only explain of `grok limits --json` note like
`SuperGrok billing poll failed for 58c5f686-427: Billing service error: Invalid or expired credentials (auth_kind=bearer, x_xai_token_auth=xai-grok-cli, upstream=PermissionDenied, reason=no auth context)`
while personal + business both show `includedUsedPct` 6.0, `sharedUnifiedPool` true, live SuperGrok business, `console.isLive` false.

No product edits. No secrets.

---

## 1. What is `58c5f686-427`?

**Truncated SuperGrok identity id**, not a chat session id and not a full UUID in the note.

- Full id comes from `supergrok_identity_id_from_auth`: **team_id** if set, else **user_id**, else auth store scope string.
- The note uses `short_id(&target.identity_id)`: first **12 characters** of that id when longer than 12.
- So `58c5f686-427` is almost certainly the start of a UUID-shaped team or user id (e.g. `58c5f686-427a-…`), shortened for the note only. It identifies **which principal’s billing poll failed**, not which chat session.

Code: `short_id` and the note format in
`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/limits_cmd.rs`
(`collect_limits_report_at` Err branch; `fn short_id`).

---

## 2. How dual SuperGrok billing poll works

### `grok limits` / `limits --json` (explicit collect)

1. Load **all** pollable SuperGrok principals: `load_supergrok_billing_poll_targets(grok_home)`.
2. For each target, call `fetch_credits_config_with_session(proxy_base, access_token, user_id)`.
3. Each target carries its own:
   - `identity_id` (team/user/scope)
   - **that principal’s JWT** from `auth.json` (`GrokAuth.key`)
   - `user_id` for the `x-userid` header
4. Success → build balance, `remember_supergrok_included_billing` / dollar extras / build usage (process cache only).
   Failure → push a note; **do not** put that principal in the live `balances` map.

### TUI / background sibling path

- `poll_and_remember_non_active_supergrok_included_billing` loads **non-active only** via
  `load_non_active_supergrok_billing_poll_targets` (everyone except `active_supergrok_identity_id`).
- Sibling failure is debug-logged as “active path unchanged”; it does not fail the active poll.

### Which JWT for which row

| Row | Token source |
|-----|----------------|
| Each principal | Its own SuperGrok session entry in `auth.json` (deduped by `identity_id`; multi-slot `::personal` / `::team::` preferred over base for the same id when both exist) |
| HTTP | `Authorization: Bearer <that JWT>`, `X-XAI-Token-Auth: xai-grok-cli` (default), `x-userid: <user_id>` |
| Endpoint | `{proxy}/billing?format=credits` (included-safe credits poll, not inference) |

Active vs sibling is only about **order / background**: limits CLI still polls **all** targets with each one’s JWT.

---

## 3. Why one principal can fail while the other still shows 6%

Polls are **independent**. One JWT can be rejected by the proxy while another is fine.

Typical causes for the failing principal:

- Stale or expired OIDC/external session for **that** identity only
- Stale multi-slot entry still listed as a distinct identity with a dead token
- Proxy/upstream: `Invalid or expired credentials` with `reason=no auth context` means the bearer was not accepted as a valid auth context (not “meter empty”)

The **successful** principal’s poll still returns included usage (here 6%). That path alone is enough to fill meters and process cache for that identity.

The failed principal does **not** get a fresh balance from this run. Rows can still **display** 6% because of shared-pool fill (next section).

---

## 4. Why both rows show the same 6% under `sharedUnifiedPool`

`sharedUnifiedPool` is true when dual SuperGrok rows are treated as one consumer pool:

- Any polled balance has `is_unified_billing_user == Some(true)`, **or**
- Two+ known included readings match on floored % and reset display.

When the shared-pool flag is on, `fill_unified_included_on_empty_slots` **copies** a known included reading onto slots that had no poll success (so personal/business do not look forever empty under unified billing). Dollar extras use a similar “fill empty from any observed prepaid” path.

So both rows at 6% with `sharedUnifiedPool: true` usually means:

1. At least one principal’s poll succeeded with ~6% included, and
2. Display fill made the other row show the **same pool**, not that both JWTs succeeded this poll.

That matches “one note failed + both meters 6%.”

---

## 5. Is the failed poll dangerous (wrong meter / wrong dual-auth rank) or soft honesty only?

**Mostly soft honesty for `grok limits --json`.**

Evidence from product code:

- Failed poll → **note only** (`notes` array). No invent of included % from the error body.
- CLI success path comments: process included cache for ranking helpers; **does not mark exhaust memos** (“read-only report path, not hop policy”).
- Sibling TUI path: failed sibling does not fail or rewrite the active billing path.
- Live sampling (`live_sampling` / business / `console.isLive false`) comes from dual-auth status + preferred method + whether SuperGrok is treated out of allowance / console-ready, **not** from “every principal poll must succeed.”

Caveats (not “ignore forever”):

- The failed identity does **not** refresh process included/dollar-extras cache for **itself**. Under a true unified pool, the successful principal’s reading is usually the same pool, so meter display is still useful.
- If the **active** identity’s token is the dead one, chat/inference can also hit `no auth context` later even while limits display looks fine from sibling fill. Operator’s case (live SuperGrok **business**, 6% warm) suggests the **live** JWT is still working; the failed short id is likely the **other** principal’s token or a stale slot for that team/user id.
- Flat multi-poll “debit proof” and similar honesty features need **healthy** session billing polls; auth-fail process ≠ debit proof (see residual notes on flat poll).

**Bottom line:** this note is **honesty about a bad credential poll**, not a hop to console and not a silent wrong meter invent. Rank/live can stay SuperGrok when the live principal is healthy.

---

## 6. What the operator should do

1. **If chat/sampling still works** (as here: live SuperGrok business, included ~6%, console not live): treat the note as **soft**. Shared pool is warm; meters are still meaningful from the good poll.
2. **To clear the note:** re-login the SuperGrok identity whose stored JWT matches the failing principal (the one whose team/user id starts with `58c5f686-427…`). Use normal `grok login` for that personal or business account; do not paste tokens into chat.
3. **Which identity:** compare doctor/auth listings fingerprints/roles. Business live + one fail often means **personal** (or a stale second slot) expired. If only one role fails in practice, re-auth that role.
4. **Do not** assume console hop or prepaid drain from this note alone. Console key path is separate (`console.isLive`).
5. **Ignore only if** you accept a stale second principal on disk and you do not care about dual-row honesty; product will keep printing the note until that token polls cleanly or is removed/refreshed via login.

---

## 7. Where the note string is built

| Piece | Location |
|-------|----------|
| Note format | `crates/codegen/xai-grok-pager/src/limits_cmd.rs` → `collect_limits_report_at` → on `fetch_credits_config_with_session` **Err**: `notes.push(format!("SuperGrok billing poll failed for {}: {e}", short_id(&target.identity_id)))` |
| Id shortening | same file, `fn short_id` (first 12 chars when len > 12) |
| Error body wrap | `crates/codegen/xai-grok-shell/src/extensions/billing.rs` → `fetch_credits_config_with_session` → non-success HTTP: `Err(format!("Billing service error: {detail}"))` where `detail` is JSON `error` string or `HTTP {status}` |
| Poll target load | `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs` → `load_supergrok_billing_poll_targets` / `SupergrokBillingPollTarget` |
| Sibling-only poll | same billing.rs → `poll_and_remember_non_active_supergrok_included_billing` (debug log, not this CLI note) |
| Shared pool fill | `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` → `dual_principals_share_unified_supergrok_pool`, `fill_unified_included_on_empty_slots` |
| Identity id meaning | `crates/codegen/xai-grok-shell/src/auth/model.rs` → `supergrok_identity_id_from_auth` |

Wire error text (`Invalid or expired credentials (auth_kind=bearer, … reason=no auth context)`) is from the **upstream billing/proxy** response body, not constructed locally beyond the `Billing service error: …` prefix.

---

## One-line parent summary

`58c5f686-427` is a **12-char prefix of a SuperGrok principal identity id** (team or user); dual limits poll each principal’s **own JWT**; one dead token fails while the other can still return 6%; **shared unified pool fill** copies that 6% onto the empty row; the note is **soft honesty**, not hop policy; re-login the failing identity if you want a clean poll, otherwise safe to treat as a stale second credential when live SuperGrok still samples fine.
