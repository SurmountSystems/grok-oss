# PR #36 just ci on 75356b20 — FAIL names

- Run: 31698013046
- Job: 94440156360
- Head: 75356b2060feaa0b78d59dce2368aeb5987e37bf
- Window: 2026-08-13T12:00:15Z–13:03:00Z (~63m)
- Step 6 `just ci-prep && just test`: 12:01:59Z–13:02:55Z, exit 100
- Summary: `[1082.594s] 29833 tests run: 29832 passed, 1 failed, 449 skipped`

## FAIL

`TRY 2 FAIL [0.185s] (18824/29833) xai-grok-shell session::acp_session::auth_retry_budget_tests::authenticated_401s_still_exhaust_after_three_retries`

MCP job tail had no assertion body (same as the prior send-now one-fail wave).

## History

This name was in the 2174fd75 42-fail list (then marked locally green). The 71bca1a0 GHA run passed it (only send-now kind routing failed). Send-now product landed in 43554621 / tip 75356b20. Now this 401 test is the only GHA fail, after nextest retry.

Node 20 deprecation / Nix cache noise only.

No GitHub writes. Reuse branch `onto-xai/b13fa526f511`.
