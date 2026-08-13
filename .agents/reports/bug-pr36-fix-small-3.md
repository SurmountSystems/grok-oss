# PR #36 three small-crate nextest fails

Date: 2026-08-13
Branch: `onto-xai/b13fa526f511`
Worktree: `/home/hunter/Projects/surmount/grok-build`
rustc: 1.97.1 (8bab26f4f 2026-07-14)
CI names from SHA `2174fd75` (`.agents/reports/bug-pr36-ci-2174fd75-fails.md`).

## Scope

Only these three:

1. `xai-grok-pager-bin::update_never_blocked_by_config corrupt_config_never_changes_update_outcome`
2. `xai-grok-pager-pty-harness::plan_approval_resume plan_approval_restored_after_resume`
3. `xai-grok-sampler retry::tests::cloudflare_edge_range_is_transient`

Did not edit `xai-grok-pager` or `xai-grok-shell`. Did not touch Nucleo `Some(2)` / `FuzzySearchManager`. Did not change the 525 range test or sampler product. SuperGrok is paid. No git.

## Outcome

| Test | After this slice |
|------|------------------|
| `corrupt_config_never_changes_update_outcome` | **Green.** Test helper now finds the `grok-oss` bin. |
| `plan_approval_restored_after_resume` | **Green.** First wait accepts fullscreen plan-approval chrome, not only the minimal card header. |
| `cloudflare_edge_range_is_transient` | **Parked.** Still red on `classify 525`. Sibling SoT says 525 is Fatal. Did not weaken the range test. Did not make 525 retryable. |

## 1. pager-bin: `corrupt_config_never_changes_update_outcome`

### Named contract

`grok update` is a recovery command. A corrupt `config.toml` must not change the update outcome versus a healthy config.

### Red (observed)

CI `a036327e` fail-detail (`/tmp/pr36-ci-a036327e/fail-detail.txt` line 26):

```
FAIL	xai-grok-pager-bin::update_never_blocked_by_config corrupt_config_never_changes_update_outcome
	crates/codegen/xai-grok-pager-bin/tests/update_never_blocked_by_config.rs:22
	PAGER_BINARY is unset and this build is not `cargo test`
```

The helper looked up `option_env!("CARGO_BIN_EXE_xai-grok-pager")`. That rustc-env is never injected. The package `xai-grok-pager-bin` ships `[[bin]] name = "grok-oss"`. Cargo documents `CARGO_BIN_EXE_<name>` with hyphens replaced by underscores, so the compile-time env is `CARGO_BIN_EXE_grok_oss`.

The update command itself already exits 0 on corrupt config once the binary is found (OSS no-updater path). This was a test-binary lookup miss, not a product `?` on config load.

### Fix

`crates/codegen/xai-grok-pager-bin/tests/update_never_blocked_by_config.rs` `pager_binary()`:

1. `PAGER_BINARY` (Bazel / CI; absolutize)
2. `option_env!("CARGO_BIN_EXE_grok_oss")` then hyphenated fallback
3. Runtime `CARGO_BIN_EXE_grok-oss` / `grok_oss` / legacy pager names if the path exists

### Green

```
cargo nextest run -p xai-grok-pager-bin --locked --test-threads=2 --build-jobs 2 \
  -E 'test(corrupt_config_never_changes_update_outcome)'
PASS [   0.556s] xai-grok-pager-bin::update_never_blocked_by_config corrupt_config_never_changes_update_outcome
```

Earlier same-session green after the lookup fix: `PASS [   0.616s]` / `PASS [   0.590s]`.

## 2. pty-harness: `plan_approval_restored_after_resume`

### Named contract

After quit + `--continue`, the shell re-parks `exit_plan_mode`. Approval chrome must come back so a side-panel Approve click leaves plan mode and starts the implement turn.

### Red (observed)

CI `a036327e` fail-detail line 28:

```
FAIL	xai-grok-pager-pty-harness::plan_approval_resume plan_approval_restored_after_resume
	crates/codegen/xai-grok-pager-pty-harness/tests/plan_approval_resume.rs:18
	shell must re-park exit_plan_mode on resume so approval chrome returns:
	restored plan-ready card after resume
```

The first wait after `--continue` asked only for `"Plan ready for review"`. That string is **minimal-mode** card chrome. Default `PtyHarness::spawn_with_content_in_dir` is fullscreen TUI. Fullscreen status is `"Waiting on plan approval"`. The side panel can already show labeled `a approve` / `s revise` or key-only `a  |  A  |  ?` while the first wait still times out at 20s.

Re-park product lives in `xai-grok-shell` (other fixer). Chrome paint lives in `xai-grok-pager` (other fixer). Allowed fix here is harness wait alignment.

### Fix

`crates/codegen/xai-grok-pager-pty-harness/src/scenarios/plan_approval_resume.rs`: first `wait_for_any_text` accepts any of:

- `"Waiting on plan approval"` (fullscreen status)
- `"Plan ready for review"` (minimal card)
- `"a approve"`, `"s revise"`, `"a  |  A  |  ?"` (panel CTA strip)

The second wait still requires a CTA before the mouse click.

### Green

```
cargo nextest run -p xai-grok-pager-pty-harness --locked --test-threads=2 --build-jobs 2 \
  -E 'test(plan_approval_restored_after_resume)'
PASS [   6.577s] xai-grok-pager-pty-harness::plan_approval_resume plan_approval_restored_after_resume
```

Earlier same-session green after the wait fix: `PASS [   6.798s]`.

## 3. sampler: `cloudflare_edge_range_is_transient` (parked)

### Named contract (SoT)

HTTP **525 is Fatal**. Sibling SoT: `classify_cloudflare_525_is_fatal_even_with_should_retry_true`. Product path:

- `SamplingError::is_retryable` uses `is_retryable_api_status` → `RetryPolicy::edge_client()`
- `edge_client` terminal list is `525, 526` (origin TLS never clears on its own)
- `classify_error` only rebuilds/retries when `err.is_retryable()`; otherwise Fatal

`is_transient_api_status` still lists `520..=527 | 530`. That helper is **not** what `classify_error` uses. Sampling-types already documents the split: transient helper includes 525/526; `is_retryable` does not.

### Red (observed, still red)

CI and local:

```
cargo nextest run -p xai-grok-sampler --locked --test-threads=2 --build-jobs 2 \
  -E 'test(cloudflare_edge_range_is_transient)'
panicked at crates/codegen/xai-grok-sampler/src/retry.rs:1016:13:
classify 525
```

The range test loops `[520, 521, 522, 523, 524, 525, 526, 527, 530]` and expects `RetryWithClientRebuild` for every code, including 525.

### Why not a product fix

Product already classifies 525 as Fatal. Making 525 retryable would **break** the sibling SoT and the 1.0.3 edge-client policy. Parent instruction: do not change the range test to accept 525 as transient. If the test is wrong versus that SoT, park it.

The range test is stale versus the Fatal SoT. Parked. No sampler source edit.

### Sibling SoT green

```
cargo nextest run -p xai-grok-sampler --locked --test-threads=2 --build-jobs 2 \
  -E 'test(classify_cloudflare_525_is_fatal)'
PASS [   0.025s] xai-grok-sampler retry::tests::classify_cloudflare_525_is_fatal_even_with_should_retry_true
```

Same FAIL on the range test in that run (expected).

## fmt / clippy

```
cargo fmt -p xai-grok-pager-bin -p xai-grok-pager-pty-harness -p xai-grok-sampler
# no remaining diffs from fmt

cargo clippy -p xai-grok-pager-bin -p xai-grok-pager-pty-harness -p xai-grok-sampler \
  --lib --bins --locked -- -D warnings
# exit 0, Finished `dev` profile in 3m 15s
```

## Files touched

- `crates/codegen/xai-grok-pager-bin/tests/update_never_blocked_by_config.rs`
- `crates/codegen/xai-grok-pager-pty-harness/src/scenarios/plan_approval_resume.rs`
- this report

## Residual for parent

`cloudflare_edge_range_is_transient` will stay red in `just ci` until someone (outside this "do not weaken" slice) deletes 525/526 from that classify loop or splits the range so origin-TLS codes are not asserted retryable. Product is already correct. Do not flip 525 to retryable.
