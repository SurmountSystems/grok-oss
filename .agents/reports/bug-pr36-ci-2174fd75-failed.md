# PR #36 `just ci` FAILED on `2174fd75`

Read-only pull. No product edits. No GitHub writes (no comment, review, re-run, dispatch, labels, push, or commit).

## Run

| Item | Value |
|------|--------|
| PR | [SurmountSystems/grok-oss#36](https://github.com/SurmountSystems/grok-oss/pull/36) |
| SHA | `2174fd75db9a814efbb704b0ae7cf0f7e9326073` |
| Branch | `onto-xai/b13fa526f511` |
| Workflow | **CI** (`.github/workflows/ci.yml`), run **#58**, `pull_request` synchronize |
| Run | [31680531078](https://github.com/SurmountSystems/grok-oss/actions/runs/31680531078) |
| Failed check / job | **just ci** ([94384792780](https://github.com/SurmountSystems/grok-oss/actions/runs/31680531078/job/94384792780)) |
| Conclusion | **failure** |
| Started / ended | 2026-08-13T08:05:31Z – 09:10:10Z (about 1h 4m) |
| Job window | 08:05:51Z – 09:10:09Z |
| First failing step | **`just ci-prep && just test`** (step 6, 08:07:13Z – 09:10:05Z, about 63 min) |
| Exit | **100** (annotation on step 6, log line 36703) |

Earlier steps succeeded: Checkout, Free disk space, Install Nix, Add swap.

## Classification

**nextest runtime** (from public job metadata only)

Not fmt. Not clippy. Not `--no-run` compile. Not test-doc / mem-guard. Not infra.

Why this class, without the log body:

- Exit **100** is cargo-nextest’s “tests failed” status. Compile / runner errors on this repo have been **101** (`cargo test --no-run` or rustc).
- Step 6 ran **~63 minutes**. That matches a full workspace nextest after fmt + clippy + `--no-run` (prior completed fail `a036327e` / run 31673700687 was ~67 minutes and printed a nextest summary). Fmt/clippy-only dies much earlier.
- The failure annotation is at log line **36703**, which is a long nextest transcript, not a short fmt/clippy dump.

Ignored noise (not the fail): Nix cache restore 400, cache save “services aren’t available,” Node 20 deprecation.

`just test` order in-tree: `test-fmt` → `test-clippy` → `test-unit` (`cargo nextest run --workspace --locked`) → `test-doc` → `test-mem-guard`. Doc and mem-guard do not run after nextest exits 100.

## First fail lines / test names

**Not available.** Public REST jobs + check-runs + annotations have no rustc or `FAIL` text. Unauthenticated `GET .../jobs/94384792780/logs` is **403** (“Must have admin rights”). The HTML job page is login-walled. This subagent had no `search_tool` / `use_tool` / `get_job_logs` in its callable set, so the signed Actions zip was not downloaded.

Do **not** treat the `a036327e` list of 86 names as this run’s list. `2174fd75` only claimed three disjoint greens (tools allexport, workspace `ensure_binding`, textarea Home/End). Remaining clusters from that older run are a hypothesis, not this log.

Parent: spawn a log puller that actually has GitHub MCP `get_job_logs` (`return_content=true`, `tail_lines` large enough to include the nextest summary) or download the signed zip. Then extract:

- the `Summary [ ... ]` / `N failed` line
- first 5–10 `FAIL` / `ABRT` names

## Reads only

No GitHub writes. No product edits. No cargo/nextest. Sources: public REST `/actions/runs/31680531078`, `/jobs`, `/check-runs/94384792780` + annotations, public Actions HTML.
