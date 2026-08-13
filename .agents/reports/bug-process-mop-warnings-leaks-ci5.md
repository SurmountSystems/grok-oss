# Process mop — warnings / leaks / CI-5 named tests

**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Agent:** L2 process mop only  
**Date:** 2026-08-13  
**Touched crates:** `xai-grok-shell`, `xai-grok-pager`  
**No product edits.** No git add / commit / push. No keyring unlock.

## Result

fmt, clippy (`--lib --bins -- -D warnings`), and the named tests all exited **0**. No fallout to mop.

---

## 1. fmt

```bash
cargo fmt -p xai-grok-shell -p xai-grok-pager
```

| Result | Exit |
|--------|------|
| ok | **0** |

---

## 2. clippy

```bash
cargo clippy -p xai-grok-shell --lib --bins -- -D warnings
```

| Result | Exit | Notes |
|--------|------|-------|
| ok | **0** | Already built; `Finished` in 0.49s |

```bash
cargo clippy -p xai-grok-pager --lib --bins -- -D warnings
```

| Result | Exit | Notes |
|--------|------|-------|
| ok | **0** | Checked shell + compiled pager + update; `Finished` in 37.52s |

---

## 3. Named tests

### 3.1 `cancel_running_task_tests`

```bash
cargo test -p xai-grok-shell --lib cancel_running_task_tests
```

| Result | Exit |
|--------|------|
| 20 passed; 0 failed; 6543 filtered out | **0** |

### 3.2 `credentials_store::tests`

```bash
cargo test -p xai-grok-shell --lib credentials_store::tests -- --test-threads=1
```

| Result | Exit |
|--------|------|
| 16 passed; 0 failed; 6547 filtered out | **0** |

### 3.3 Named offline shell tests

```bash
cargo test -p xai-grok-shell --lib --offline -- --test-threads=1 \
  post_auth_settings_xai_upgrades_writeback_emits_and_opens_gate \
  post_auth_settings_non_xai_keeps_local_but_still_emits \
  post_auth_settings_failure_resolves_gate_onto_local_policy \
  settings_not_cached_when_identity_logs_out_during_fetch \
  terminal::local_terminal::tests::test_timeout_kills_grandchildren_and_returns_promptly
```

| Result | Exit |
|--------|------|
| 5 passed; 0 failed; 6558 filtered out | **0** |

Note: `test_timeout_kills_grandchildren_and_returns_promptly` printed `kill: sending signal to <pid> failed: No such process` and still passed. That is the test's own kill of an already-reaped grandchild, not a harness failure.

### 3.4 Pager history search

```bash
cargo test -p xai-grok-pager --lib views::history_search::tests -- --test-threads=1
```

| Result | Exit |
|--------|------|
| 16 passed; 0 failed; 8738 filtered out | **0** |

---

## Fallout

None. No source edits in this mop.
