# OAuth 403 `bad-credentials` diagnosis and fix

**Date:** 2026-08-09
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Symptom:** Mid-session turn failed in ~0.4s with yellow "Retry failed" and
`Turn failed ... Internal error: { json }` while free SuperGrok period chrome
still showed ~22% used.

**Wire body (operator):**
```
API error (status 403 Forbidden): unauthenticated:bad-credentials: The OAuth2 access token could not be validated.
```

---

## Operator: what to do NOW (no secrets)

This is **invalid / unvalidated OAuth credentials**, not free SuperGrok period
limits running out (the ~22% chrome is unrelated).

1. **In the TUI (preferred):** run **`/login`** and complete the browser sign-in.
   After success, resend the failed message (product may offer re-auth stash
   auto-resubmit when re-auth prompt is shown).
2. **Or clear then sign in again from a shell:**
   ```bash
   grok logout
   grok login
   ```
   `logout` clears the active SuperGrok session cache; `login` opens the
   browser OAuth flow and writes a new session. Neither command prints tokens
   or keys in normal use.
3. **If you use two SuperGrok logins** (personal + Business): check **`/doctor`**
   or **`/limits`** for which role failed auth, then **`/login`** (or
   `grok login`) for **that** SuperGrok account. Multi-slot siblings stay until
   you log them out.
4. **If you only need console API spend:** ensure a console key is available
   (`XAI_API_KEY` or `grok login --api-key`) and that free SuperGrok period is
   actually full if you expect console primary. For this exact error, re-auth
   of the SuperGrok session is still the right first step when you intend to
   stay on SuperGrok.
5. **Do not** paste access tokens, refresh tokens, or API keys into chat or
   tickets. **Do not** invent free SuperGrok period % from this error.

After this product fix is in the binary you run: the same failure should show
an **Authentication required** / re-auth prompt (`/login`) instead of Internal
error JSON, and the product will **try one OIDC refresh** before giving up
(session-based SuperGrok only).

---

## Diagnosis (code)

### What the gateway returned

HTTP **403 Forbidden** with body wording:

- `unauthenticated:bad-credentials`
- `The OAuth2 access token could not be validated`

That is **credential rejection**, not content-safety / ZDR policy and not team
credits.

### Where product used to map it

| Layer | Prior behavior | Why it hurt |
|-------|----------------|-------------|
| `xai-grok-sampler` client | Only **401** → `SamplingError::Auth`. **403** always → `SamplingError::Api` | No auth recovery path |
| `SamplingError::is_auth_error()` | Only Auth variant or **401** Api | Retry classifier treated 403 as Fatal, not EmitToSession |
| `handle_sampling_failure` | Refresh only for Auth kind / status **401** | No `try_recover_unauthorized` on this 403 |
| `map_sampling_err_to_acp` | 403 → `internal_error` (credit body special-cased) | ACP "Internal error" envelope |
| Pager `format_acp_error` | `err.to_string()` + sanitize | Yellow "Internal error: …" / raw-ish dump |
| `is_reauthable_failure` | Only `error_type == "auth"` or `"Unauthorized (401)"` substring | **Retry failed** with body, not **ReAuthRequired** |

Intentional historical rule (still true for **policy** 403): bare 403 and
content-safety / ZDR must **not** fire OIDC refresh or wipe `auth.json`. That
rule was over-broad for **credentials-rejected** bodies on 403.

### What this is **not**

- **Not** free SuperGrok period limits exhausted (chrome ~22% is consistent with
  remaining free period; this body is token validation).
- **Not** team credit / monthly spending-limit hop (no credit wording; separate
  classifier `is_credit_exhausted_message`).
- **Not** console key sticky Team JWT hop as the primary story: recovery is
  SuperGrok **SessionToken** refresh via AuthManager when the session is
  session-based; if refresh fails, operator re-login.

### Refresh path (after fix)

1. Client maps 403 + credentials body → `SamplingError::Auth` (same class as 401).
2. Sampler retry: `EmitToSession` (not soft HTTP retry loop).
3. `handle_sampling_failure`: session gate → `try_recover_unauthorized` (OIDC
   refresh) once → resubmit; on failure → RetryState `error_type: "auth"`.
4. Pager: `is_reauthable_failure` → **ReAuthRequired** (`/login` copy); PromptResponse
   suppresses redundant Turn failed when re-auth is already shown.

---

## Product fix (landed this turn)

Body-based credentials rejection, TDD red→green style contracts.

### 1. `xai-grok-sampling-types`

- New `is_credentials_rejected_message` (`bad-credentials`, OAuth token could
  not be validated, …).
- `is_auth_error()` true for **403 + credentials body** (and not credit body).
- Policy / bare 403 still **not** auth.
- Test: `forbidden_bad_credentials_is_auth_error`.

### 2. `xai-grok-sampler`

- Client: all chat/responses/messages success-check paths treat forbidden
  credentials rejection as `SamplingError::Auth` (+ 401 attribution breadcrumb).
- `SamplingErrorInfo`: Api with `is_auth_error()` → kind **Auth** (recovery
  eligible even if status stays 403).
- Retry: 403 bad-credentials → EmitToSession; policy 403 stays Fatal.
- Tests: `api_403_bad_credentials_classified_as_auth`,
  `classify_forbidden_bad_credentials_emits_to_session`,
  `classify_forbidden_policy_is_fatal_not_auth`.

### 3. `xai-grok-shell`

- `map_sampling_err_to_acp`: 403 credentials → `auth_required` (not Internal
  error). Policy 403 and credit 403 paths unchanged.
- `is_reauthable_failure`: also matches credentials-rejected message text.
- Test: `forbidden_bad_credentials_maps_to_auth_required`.

### 4. `xai-grok-pager`

- RetryState tests: 403 bad-credentials with `error_type: "api"` still pushes
  **ReAuthRequired**.
- `is_reauthable_failure_matrix` extended.

### Explicit non-goals

- No free SuperGrok period % invention or hop-meter changes.
- No host keyring safari; diagnosis was code + tests only.
- No git commit / add / push.

---

## Verify (ran)

```text
cargo test -p xai-grok-sampling-types --lib credentials|forbidden|unauthorized_is_auth
cargo test -p xai-grok-sampler --lib api_403_bad_credentials|classify_forbidden|retry
cargo test -p xai-grok-shell --lib forbidden_bad_credentials|forbidden_does_not_map|sampler_401_recovery
cargo test -p xai-grok-pager --lib reauthable|apply_retry_state_403_bad|apply_retry_state_auth
cargo fmt -p xai-grok-sampling-types -p xai-grok-sampler -p xai-grok-shell -p xai-grok-pager
cargo clippy -p xai-grok-sampling-types -p xai-grok-sampler -p xai-grok-shell -p xai-grok-pager -- -D warnings
```

Product libs clippy clean. `--all-targets` on shell/pager still hits **pre-existing**
test-only clippy noise in other modules (not introduced here).

---

## Files touched

- `crates/codegen/xai-grok-sampling-types/src/error.rs`
- `crates/codegen/xai-grok-sampling-types/src/lib.rs`
- `crates/codegen/xai-grok-sampler/src/client.rs`
- `crates/codegen/xai-grok-sampler/src/events.rs`
- `crates/codegen/xai-grok-sampler/src/retry.rs`
- `crates/codegen/xai-grok-shell/src/sampling/error.rs`
- `crates/codegen/xai-grok-shell/src/extensions/notification.rs`
- `crates/codegen/xai-grok-pager/src/app/acp_handler/tests/session_events.rs`

---

## Residual

- Sticky Team JWT vs SuperGrok session token **which bearer** was on the wire
  for the dogfood turn was not reconstructed from logs here (no session
  transcript in this task). Fix is correct for any first-party path that returns
  this body.
- Optional later: terminal ACP after failed recovery still uses
  `internal_error` envelope while RetryState drives UX; reauth suppress already
  covers the bad chrome. Could return `auth_required` ACP for consistency.
