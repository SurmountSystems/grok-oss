# PR #36 `just ci` fail on `82fa1794`

Date: 2026-08-13. Branch: `onto-xai/b13fa526f511`. Read-only. No product edits. No GitHub write.

## Run

| Item | Value |
|------|--------|
| Run | [31668960010](https://github.com/SurmountSystems/grok-oss/actions/runs/31668960010) (CI #53, `pull_request` synchronize) |
| Job | [94349508279](https://github.com/SurmountSystems/grok-oss/actions/runs/31668960010/job/94349508279) `just ci` |
| SHA | `82fa1794a8f1751045da6eb85b3e43d902972a69` |
| Conclusion | **failure** |
| Duration | 31m 46s (job 05:02:46Z to 05:34:32Z) |
| Failed step | `just ci-prep && just test` (step 6, 05:04:21Z to 05:34:29Z) |
| Exit | **101** (annotation at step log line 6336) |

Noise that was **not** the fail: Nix cache restore 400, Nix cache save outage HTML, Node 20 deprecation.

## First real fail

**Not in public annotations.** The only failure annotation is `Process completed with exit code 101`. Job log download via unauthenticated API returns 403 (`Must have admin rights to Repository.`). HTML job page is `Sign in to view logs`. This explore agent has no `gh` / shell / working MCP invoke, so the first rustc or nextest line is not on disk here.

What the public record still proves:

- Not `test-fmt`. Fmt uses rustfmt and historically exits **1**. Prior fmt fail on this restack was ~4 minutes.
- Not infra-only. The cargo step ran ~30 minutes after Nix.
- Duration matches prior **test-unit** fails on this restack (~30 to 36 minutes after fmt+clippy already passed), not the clippy fail (~11 minutes).
- Current `just test-unit` is `cargo nextest run --workspace --locked` (not a separate `cargo test --no-run` recipe). Nextest both compiles and runs tests. Later recipes are `test-doc` and `test-mem-guard`.
- Tip `82fa1794` only changed `crates/codegen/xai-grok-shell/src/agent/config.rs` (make `inject_url_derived_headers` public and fold OpenRouter attribution headers). That is the fix for the previous run's compile error `E0603` in `openrouter_attribution.rs:7`. That compile error is not expected to have regressed.

## Classification

**test-runtime** (most likely), else **test-compile** if the local `--no-run` green was the mop tree rather than this SHA.

Not fmt. Not clippy (timing + tiny lib delta after a green clippy tip). Not infra-noise.

Likely leftover named contract if nextest actually ran: `xai-grok-pager-minimal::overlay::tests::question_input_mode_editor_grows_and_keeps_row_prefix` in `crates/codegen/xai-grok-pager-minimal/src/overlay.rs:959`. Prior PR36 reports said this test was **not** on those CI fail lists because the job died earlier in compile. That is a hypothesis, not a log line.

## Would local `--no-run` have caught it?

**No**, if the GHA fail is nextest **runtime** (or `test-doc`). `cargo test --no-run --workspace --jobs 2 --locked` only compiles test binaries.

**Yes**, if GHA died compiling a test binary on this exact SHA. Local `--no-run` exited 0 about 11 minutes after this run started, while a compile mop was already editing the shared workspace, so that 0 is not proof that GHA's 82fa1794 tree compiled.

## Suggested smallest fix

1. Authenticated: `gh run view 31668960010 --repo SurmountSystems/grok-oss --log-failed` (do not hang; use `--log-failed` / `gh api` if `gh run view` stalls).
2. If the first product line is a nextest **FAIL** on the overlay editor-height / `z (·)` prefix contract: make `render_question` keep the freeform row prefix and grow the editor by one row per extra line. Do not weaken the asserts.
3. If the first product line is still `error[E0…]` / `could not compile`: treat it as the next restack compile hole (same class as private helper / missing dev-dep / missing field). Fix that crate only.

Do not re-fix rustfmt unused-binding, shell/pager clippy, `SamplingError::is_overloaded`, `pretty_assertions`, or `inject_url_derived_headers` unless the new log shows they regressed.

## Commands

No `gh` (this agent has no shell). Read-only HTTP:

- `GET https://api.github.com/repos/SurmountSystems/grok-oss/actions/runs/31668960010`
- `GET .../actions/runs/31668960010/jobs`
- `GET .../actions/jobs/94349508279`
- `GET .../actions/jobs/94349508279/logs` → 403 admin rights
- `GET .../check-runs/94349508279` and `.../annotations`
- `GET .../actions/runs/31668960010/artifacts` → empty
- `GET .../commits/82fa1794a8f1751045da6eb85b3e43d902972a69`
- HTML run/job pages (sign-in wall on logs)
- Local reads: prior `.agents/reports/bug-pr36-just-ci-*.md`, `justfile`, `.github/workflows/ci.yml`, `.config/nextest.toml`, `overlay.rs` test at line 959
