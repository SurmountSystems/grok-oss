# Process mop — CI residual wave (2026-08-12)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 process mop
**No git commit.**

## Goal

fmt → clippy (-D warnings, package-scoped) → spot reverify after pager/shell/oneshot residual greening.

---

## 1. fmt

```bash
nice -n 19 ionice -c3 cargo fmt -p xai-grok-pager -p xai-grok-shell \
  -p xai-grok-pager-minimal -p xai-grok-tools -p xai-grok-agent \
  -p xai-grok-sampler -p xai-grok-update -p xai-grok-pager-render
# + re-fmt after mop: xai-grok-tools, xai-grok-pager-pty-harness, xai-grok-pager-render
```

| Result | Exit |
|--------|------|
| **ok** (no dirty fmt left on listed packages) | **0** |

---

## 2. clippy (`--all-targets -- -D warnings`)

### Green after this mop

| Package | Exit | Notes |
|---------|------|-------|
| `xai-grok-tools` | **0** | Re-applied 2026-08-11 enroll/dead-code/test-lock mop (regressed on onto) |
| `xai-grok-pager-pty-harness` | **0** | Re-enrolled `spawn_command`; re-exported oauth seeds + `keys::{RIGHT,F2}` for e2e compile |
| `xai-grok-pager-render` | **0** | Re-enrolled clipboard/link_opener std spawns; `manual_range_contains` on glyphs blink half |

### Blocked (not thrashed)

| Package | Exit | Why |
|---------|------|-----|
| `xai-grok-shell` | **101** | **~50** errors under `-D warnings`: majority `unreachable_pub` (usage_log + others), plus 2 disallowed `tokio::process::Command::spawn`, 1 private-type-in-public (`ModelByok`), 1 unexpected `cfg` (`shell-half-merge-tests`), 1 needless_ref. **Too large for process-mop thrash** — needs a dedicated clippy slice. |
| `xai-grok-pager` (`--all-targets` and `--lib`) | **101** | Depends on shell path; fails when shell lib fails under `-D warnings`. Not a separate pager-source lint surface in this run once deps were mopped. |

Host still prints build-script warning that `tokio::process::Command::spawn` is not a reachable path in `clippy.toml` (pre-existing; does not fail `-D warnings` once packages compile).

### Mop product/test-harness fixes (surgical re-apply)

| Area | Files |
|------|-------|
| Dead helper removed | `xai-grok-tools` `computer/local/terminal.rs` (`ensure_persistent_shell_initialized`) |
| Git probe enroll | `xai-grok-tools` `util/implement_memory/workspace.rs` |
| Lifecycle test enroll | `xai-grok-tools` `computer/local/lifecycle.rs` |
| single_match / await_holding_lock / len_zero | `shared_http_rate_limit.rs`, `image_gen/mod.rs`, `opencode/edit/mod.rs`, `use_tool/mod.rs`, `session_reader/mod.rs` |
| PTY harness enroll + e2e exports | `xai-grok-pager-pty-harness` `pty.rs`, `lib.rs` |
| OS helper enroll | `xai-grok-pager-render` `clipboard/mod.rs`, `link_opener.rs`, `glyphs.rs` |

---

## 3. Spot reverify (max nice)

### Pager full lib

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib -- --test-threads=8
```

| Result | Exit | Wall (approx) |
|--------|------|----------------|
| **8813 passed; 0 failed; 11 ignored** | **0** | ~13.2s |

Matches residual resample claim (`bug-pager-lib-residual-resample-2026-08-12.md`).

### Shell residual filters (substring clusters, threads=1)

```bash
RUST_MIN_STACK=16777216 nice -n 19 ionice -c3 cargo test -p xai-grok-shell --lib <filter> -- --test-threads=1
```

| Filter | Passed | Exit |
|--------|-------:|------|
| `mcp_reenable` | 6 | 0 |
| `plan_mode_rejects` | 2 | 0 |
| `queue_input_send_now` | 3 | 0 |
| `recap_display_only` | 26 | 0 |
| `auth_retry` | 16 | 0 |
| **Total sample** | **53** | **all 0** |

Note: multi-name `|` regex as a single cargo filter matched **0** tests on this cargo (substring filters only); ran clusters one filter at a time.

### pager-minimal

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager-minimal --all-targets
```

| Result | Exit |
|--------|------|
| **86 passed; 0 failed** (includes dim-rail + plan insert) | **0** |

---

## 4. Bottom line

| Track | Status |
|-------|--------|
| Unit residual greened this wave (pager lib, shell named clusters, oneshots, pager-minimal) | **Holds green** under spot reverify |
| fmt on listed packages | **Green** |
| clippy deps that blocked `-D warnings` (tools, pty-harness, pager-render) | **Mopped green** |
| clippy `xai-grok-shell` / thus full pager package under `-D warnings` | **Still red** (~50 shell lints; report-only) |

**Next process residual (not done here):** dedicated shell clippy pass for `unreachable_pub` + remaining spawn enroll sites so `cargo clippy -p xai-grok-pager|shell --all-targets -- -D warnings` can finish green.
