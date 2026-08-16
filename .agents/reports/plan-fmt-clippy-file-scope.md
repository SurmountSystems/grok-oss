# File-scoped rustfmt, clippy, and tests vs crate-wide process law

Read-only map for the plan that would stop SSD-thrashing `cargo fmt -p <crate>` and `cargo clippy -p <crate> --all-targets` on this ~2M-line workspace. No cargo was run. No product files were edited.

Process note: this L2 did not have `spawn_subagent` in its tool list, so the reads below were done on this layer. Claims are from files on disk, not from a live cargo probe.

## 1. Process law that forces crate-wide cargo and a backup mop

### Project `AGENTS.md` hard constraints 3a and 3b

Path: `/home/hunter/Projects/surmount/grok-build/AGENTS.md`

3a mandates package-scoped fmt, **crate-wide clippy with `--all-targets`**, and targeted tests:

> 3a. **Post-impl verify (fmt + clippy + tests; pinned 2026-08-04).** When you create or edit product code (especially `*.rs`), before report / handoff: (1) **`cargo fmt -p <crate>`** for touched packages (not bare `rustfmt`; workspace is **edition 2024**); (2) **`cargo clippy -p <crate> --all-targets -- -D warnings`** (or package-equivalent) on those packages; (3) targeted tests for contracts you touched (`cargo test -p …` / nextest filters).

3b mandates a **second** L2 mop after the implementer already ran those steps (backup, not a skip license):

> 3b. **Effort ≥ 2 process mop.** After the primary implementer finishes (and after fix rounds that change code), L1 spawns a **process mop** L2 (`[process]` / `[process-mop]`). That L2 spawns L3 specialists to run fmt → clippy → relevant tests and **mop** fallout. … The implementer track still has L3 run the three steps first; mop is backup.

### Host `~/.grok/AGENTS.md` § Post-impl verify

Path: `/home/hunter/.grok/AGENTS.md` (pinned 2026-08-04)

The table names the same crate-wide clippy form and forbids bare rustfmt:

> **1. fmt** | `cargo fmt -p <crate>` (touched packages) | Not bare `rustfmt path.rs` on **edition 2024** (let-chains). Prefer package-scoped over full `cargo fmt --all` every micro-edit.
> **2. clippy** | `cargo clippy -p <crate> --all-targets -- -D warnings` … on **touched** packages | Catches `cfg(test)`-only APIs used in product code, dead methods, etc.
> **3. tests** | Targeted `cargo test -p <crate> --lib <filter>` / nextest for **contracts you touched or added**

The mop paragraph is explicit that the implementer must already have run the three steps, then a **separate** L2 repeats them:

> **Effort ≥ 2 — process mop (floor sweeper):** after the primary implementer finishes … spawn a **separate** L2 whose **only** job is to coordinate: fmt → clippy → relevant tests on touched packages, then **mop** any failures … Primary implementer still **must** run the three steps first … the mop is backup, not a license to skip.

### Implement skill: Step 1b and orchestrator verify gate

Path: `/home/hunter/.agents/skills/implement/SKILL.md`

Implementer summary contract (crate-wide clippy again):

> **Post-impl verify (mandatory in summary):** before declaring done, for every package you edited: (1) `cargo fmt -p <crate>`; (2) `cargo clippy -p <crate> --all-targets -- -D warnings` (or project package clippy); (3) targeted `cargo test` / nextest …

Orchestrator **resume** gate (this is the useful half):

> **Orchestrator gate (post-impl verify):** if the summary lacks evidence of **fmt + clippy + relevant tests** on touched Rust packages (or non-Rust equivalents), **resume the implementer once** to run them and re-write the summary. Do not start review on a known unlinted tree.

Step 1b is the **mandatory extra wave** at effort ≥ 2:

> ### Step 1b: Process mop (effort ≥ 2 only)
> When `effort >= 2` and the implementer (or a fix round) changed product code, spawn a **process mop** before review (and again after fix rounds before re-review) …
> Effort = 1: no separate mop (implementer owns all three steps; orchestrator gate still requires summary evidence).

Fix rounds re-run the mop (`Effort ≥ 2: re-run Step 1b process mop after fix if product code changed`). Board ids: `impl:process-mop`, `impl:process-mop-fix-N`.

### Law vs this repo's own CI

`just test-clippy` in `/home/hunter/Projects/surmount/grok-build/justfile` is **not** `--all-targets`:

> Not --all-targets on clippy: unit/integration tests pull cross-crate `cfg(test)` seams …
> `cargo clippy --workspace --lib --bins --locked -- -D warnings`

Process law is **stricter and more expensive** than `just test`. The `--all-targets` mandate is host/skill prose, not the project's quality gate. `just test-fmt` is `cargo fmt --all -- --check` (workspace-wide check, not what implementers should run on every slice).

## 2. Residual: `feat:agentic-fmt-clippy-acp` and §2h

### Original intent of agentic ACP fmt/clippy

`RESIDUAL.md` Open (2026-08-09), board `feat:agentic-fmt-clippy-acp`:

The standing note that implementers must run `cargo fmt` (and clippy + targeted tests) on touched packages **is not working reliably**. Do not add more prose scolds. Treat it as a **product + process feature**: a more **agentic ACP** approach so format/lint/test mop is **structural** (tooling, hooks, post-turn effects, a real mop contract, or host/product integration that actually runs), not a chat checklist agents skip under load. Design was deferred past the dogfood wave. Until product makes it stick, 3a/3b remain the intended bar.

That intent is **make verify actually happen**, not **run crate-wide cargo twice**. File-scoped structural verify would still satisfy the residual if the host or ACP actually ran it.

### Residual §2h (different item)

`RESIDUAL.md` 2h, board `plan:structured-token-efficient-convo` (OPEN, park full plan):

Stop loose main-thread marathons. Want a **deliberate conversation structure**: parent coordinator only, subagents own research/edits, short reports on disk, board + residual for memory, when to plan vs implement, short plain-English status. Complements host subagent token strategy. Not a fmt/clippy spec. Do not invent a product conversation protocol here.

Two residual items: 2h is session-structure efficiency; `feat:agentic-fmt-clippy-acp` is making fmt/clippy/test **run for real**. A file-scoped structural hook serves both (fewer tokens, fewer disk writes).

## 3. File-scoped rustfmt via cargo (exact argv)

Workspace:

- `/home/hunter/Projects/surmount/grok-build/rustfmt.toml` has only `use_field_init_shorthand = true`. **No `edition` key. No `max_width` override** (rustfmt default 100).
- Edition **2024** is `[workspace.package]` in `/home/hunter/Projects/surmount/grok-build/Cargo.toml`.
- Toolchain: `/home/hunter/Projects/surmount/grok-build/rust-toolchain.toml` channel `1.97.1`, components `rustfmt` and `clippy`.
- AGENTS forbids **bare** `rustfmt path.rs` because edition 2024 let-chains need rustfmt invoked with that edition. `cargo fmt -p <crate>` without paths walks **every** `.rs` in the package. `xai-grok-pager` alone is hundreds of Rust files.

`cargo fmt` forwards tokens after `--` to rustfmt. Files are rustfmt positional arguments.

**Preferred implementer argv (explicit edition, one or more edited files):**

```bash
cargo fmt -- --edition 2024 --config-path rustfmt.toml \
  crates/codegen/xai-grok-pager/src/path/to/edited.rs \
  crates/codegen/xai-grok-pager/src/path/to/other.rs
```

Run from the workspace root so `--config-path rustfmt.toml` and `rust-toolchain.toml` resolve. Multiple files on one argv is one rustfmt process.

**Check-only (CI-shaped, still file-scoped):**

```bash
cargo fmt -- --edition 2024 --config-path rustfmt.toml --check \
  crates/codegen/xai-grok-pager/src/path/to/edited.rs
```

**Gotchas (not re-probed this turn):**

1. `cargo fmt -p xai-grok-pager` without file paths still formats the whole crate. That is the SSD problem.
2. `cargo fmt -p xai-grok-pager -- file.rs`: cargo-fmt historically treats post-`--` as rustfmt args. Package selection may be ignored once files are listed. Do not rely on `-p` to inject edition when files are present. Pass `--edition 2024` after `--`.
3. Bare `rustfmt file.rs` (forbidden) defaults to an older edition when `rustfmt.toml` has no `edition`, which is how let-chains get mangled or rustfmt errors.
4. rustfmt is **not** a typecheck. It only reads the listed files plus config. That is why file-scoped fmt is cheap and honest.
5. Generated or third-party trees under `third_party/` should not be in the file list unless that was the edit.

## 4. Clippy: cheapest honest command (no `--all-targets`)

Clippy **must typecheck** the selected cargo targets. There is no real "lint this one file" that still sees types, impls, and `cfg(test)` seams without compiling a target that includes that file.

### What `-- <file>` does

`cargo clippy -p <crate> -- path.rs` does **not** restrict linting to that file. Everything after `--` is **rustc/clippy flags**. A path is an extra rustc input or an unknown flag. It is not a file filter. Do not teach `-- file.rs` as file-scoped clippy.

### Cheapest honest ladder (this repo)

Match **`just test-clippy`**, then shrink the package and target:

1. **Edited lib code (usual):**
   ```bash
   cargo clippy -p <crate> --lib --locked -- -D warnings
   ```
   Typechecks the library only. Incremental if `CARGO_TARGET_DIR` is warm.

2. **Edited a binary in that crate:**
   ```bash
   cargo clippy -p <crate> --bin <bin-name> --locked -- -D warnings
   ```
   Or `--bins` if several bins changed.

3. **Need the law's `cfg(test)`-only API catch** (clippy on test targets):
   ```bash
   cargo clippy -p <crate> --lib --tests --locked -- -D warnings
   ```
   Cheaper than `--all-targets` (skips benches/examples) but still compiles test crates. Only when the edit is in `#[cfg(test)]` or an integration test.

4. **Do not default to** `cargo clippy -p <crate> --all-targets`. That is current law and the SSD thrash. This workspace's own justfile **refuses** `--all-targets` because integration tests pull Bazel `cfg(test)` seams.

`clippy-driver` is rustc with clippy. Same typecheck cost, worse UX (you must pass the crate graph yourself). Not cheaper.

`rust-analyzer` check-on-save can feel file-scoped but still typechecks the crate in RA's own target dir. Agents must not depend on a live RA process.

### Incremental and `CARGO_TARGET_DIR`

Repo `justfile` / docs do **not** set `CARGO_TARGET_DIR`. A search of `*.md`, `*.toml`, `justfile`, `*.nix` found no `grok-build-target`. Operator practice `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target` is host env. Incremental **does** make repeat `--lib` clippy of the same crate cheap after the first typecheck. It does **not** make `--all-targets` on `xai-grok-pager` cheap on a cold or rustc-bumped cache, and it does not skip unused test/bench graphs.

Honesty: first clippy of a large crate after a toolchain bump still walks the crate graph. File-scoped fmt is the only command that stays proportional to files touched.

## 5. Tests: derive a filter from a `*.rs` path (heuristic, not a map)

Process law already says **targeted** tests, not `nextest --workspace`. `just test-unit` is workspace nextest. Implementers must not run that as post-impl verify.

There is **no** repo map from product `.rs` → tests that exercise it. You cannot honestly say "tests for this file" for an arbitrary product module.

**Heuristic only:**

| Changed path | Filter |
|--------------|--------|
| `crates/.../<pkg>/src/**/*.rs` with an inline `mod tests` / `#[cfg(test)]` | `cargo test -p <pkg> --lib <rust_mod_path>` where `<rust_mod_path>` is the module path (`app::dispatch::foo` or `foo::tests`). Stem-only filter (`foo`) over-matches. |
| `crates/.../<pkg>/src/**/tests.rs` or `.../tests/mod.rs` | Same `--lib` with that module path. |
| `crates/.../<pkg>/tests/<name>.rs` (integration) | `cargo test -p <pkg> --test <name>` |
| `crates/.../<pkg>/src/bin/<name>.rs` or `bins` | `cargo test -p <pkg> --bin <name>` |
| Nextest equivalent | `cargo nextest run -p <pkg> --lib <filter>` or `-E 'binary(id) + test(filter)'`. Still **package + filter**, never `--workspace`. |

If the file has no `#[cfg(test)]` and is not an integration/bin test, the honest verify is: run the **named tests you added or changed**, or skip tests for that file and say so. Do not invent a workspace nextest run to fill the checkbox.

`.config/nextest.toml` only sets fail-fast/retries. No per-file test groups.

## 6. Economic mode effort ceiling 3 vs live spawn cap (they are different)

| Knob | What it is | Default | Hard refuse? |
|------|------------|---------|--------------|
| Implement-loop **effort** under economic mode | Reviewer **fan-out** 1–5 for `/implement` (Token Economy). Not reasoning `/effort`. | Ceiling **3**, desired inject **2**, floor 1, lock 0. Master: `[ui] economic_mode` + `cap_implement_effort_when_economic`. | Product **rewrites** `--effort` before the skill runs. It does **not** refuse `spawn_subagent`. |
| Live **session** spawn cap | How many non-workflow child sessions may run at once. | `DEFAULT_MAX_CONCURRENT = 32` in `crates/codegen/xai-grok-tools/.../task/admission.rs`. Default behavior **`Queue`**. | **Refuse only** if `limit_behavior = "fail"` / `GROK_SUBAGENT_LIMIT_BEHAVIOR=fail` **and** `running >= max_concurrent`. Model text: "Concurrent subagent limit reached… Do not retry." |
| Depth | L1→L2→L3 nesting. | `DEFAULT_MAX_DEPTH = 2`. Env `GROK_SUBAGENTS_MAX_DEPTH`. | **Yes:** `spawn_subagent` fails when `depth >= max_depth` (no L4). |
| Master disable | `[subagents] enabled = false` or `GROK_SUBAGENTS=0`. | Enabled. | **Yes:** no spawns. |
| Workflow pool | Separate from session Task spawns. | `DEFAULT_WORKFLOW_MAX_CONCURRENT_AGENTS = 32`, then clamped to machine parallelism. | Workflow admission is its own semaphore, not the Task `Fail` path. |

FORK: economic mode (200k context soft-cap) is **separate from** Token Economy implement-effort caps. User-guide `05-configuration.md` application order is lock → desired inject → min floor → max ceiling. None of that is a live spawn refuse.

**Where a hard spawn refuse is live:**

1. Child depth already at `max_depth` (default 2): L3 cannot spawn L4.
2. `GROK_SUBAGENT_LIMIT_BEHAVIOR=fail` or `[subagents] limit_behavior = "fail"`, and 32 (or configured) children are already running.
3. Subagents disabled.
4. That agent type toggled off (`[subagents.toggle]`).

Default host is **queue at 32**, not refuse. Implement effort 3 (implementer + mop + up to 3 reviewers + merge, each with L3s) is far below 32. They are different meters.

## 7. Recommend

**Delete the mandatory process-mop wave.** Keep **resume-the-implementer-once** when the summary lacks fmt + clippy + targeted-test evidence. The mop exists because chat checklists were skipped (`feat:agentic-fmt-clippy-acp`). A second crate-wide `--all-targets` on effort ≥ 2 doubles the SSD cost and still does not make agents run verify. One implementer pass plus an orchestrator resume gate is the cheap honesty bar. Structural ACP later can *replace* both checklists; until then, do not pay a backup crate-wide clippy.

**Process pin text (one paragraph, for 3a/3b + host Post-impl verify + implement skill 1b):**

Post-impl verify is one implementer pass on **files and contracts you touched**, not a crate-wide or workspace cargo. Format with `cargo fmt -- --edition 2024 --config-path rustfmt.toml <edited.rs>…` (never bare `rustfmt`; never `cargo fmt -p <crate>` unless you intend to rewrite the whole package). Lint with `cargo clippy -p <crate> --lib --locked -- -D warnings` (add `--bin <name>` or `--bins` if you edited binaries; add `--tests` only when the edit is test-target code). Do not pass a source path after `--` to clippy; that is not a file filter. Do not use `--all-targets` unless you are reproducing a named test-target clippy miss. Tests are `cargo test -p <crate> --lib <module filter>` or `--test <integration-name>` for tests you added or changed, never `nextest --workspace`. If the implementer summary omits command plus exit code for those three, the orchestrator resumes that implementer once. There is no mandatory second process-mop L2. Effort ≥ 2 still means more reviewers, not a second fmt/clippy storm.

## Sources (absolute paths)

- `/home/hunter/Projects/surmount/grok-build/AGENTS.md` (3a, 3b)
- `/home/hunter/.grok/AGENTS.md` (§ Post-impl verify)
- `/home/hunter/.agents/skills/implement/SKILL.md` (Step 1 summary, orchestrator gate, Step 1b)
- `/home/hunter/Projects/surmount/grok-build/RESIDUAL.md` (Open agentic ACP bullet; §2h)
- `/home/hunter/Projects/surmount/grok-build/rustfmt.toml`
- `/home/hunter/Projects/surmount/grok-build/Cargo.toml` (`edition = "2024"`)
- `/home/hunter/Projects/surmount/grok-build/rust-toolchain.toml`
- `/home/hunter/Projects/surmount/grok-build/justfile` (`test-fmt`, `test-clippy`)
- `/home/hunter/Projects/surmount/grok-build/clippy.toml`
- `/home/hunter/Projects/surmount/grok-build/.config/nextest.toml`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-tools/src/implementations/grok_build/task/admission.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/config/mod.rs` (max_depth, env names)
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/docs/user-guide/05-configuration.md` (Token Economy effort)
- `/home/hunter/Projects/surmount/grok-build/FORK.md` (economic mode vs implement-effort)
