# Skills Python restore (restack land class)

L2 implementer report. Product skills must stay markdown plus native tools.
Agents must not add `.py` helpers. A restack that ships non-excepted Python,
or drops the Rust intercept, is a failed land.

Did not copy operator swearing. Did not rewrite host `~/.agents/skills`.
Did not invent a project `.grok/` skills home.

## What Python was found

### Product tree (this repo)

No project skill roots exist (no `.agents/skills`, no `.grok/skills`).
Product does not ship skill `.py` in git.

Three non-skill `.py` files remain in the repo. They are not product skills
and were left alone:

| Path | Class |
|------|--------|
| `crates/codegen/xai-grok-agent/scripts/encrypt_templates.py` | Codegen helper, not a skill |
| `crates/codegen/xai-grok-hooks/examples/hooks/bin/no-recursive-grep-guard.py` | Hook example, not a skill |
| `crates/codegen/xai-grok-shell/tests/memory_integration/run_tests.py` | Test helper, not a skill |

### Network cache `~/.grok/bundled/skills` (63 `.py` today)

This is what 1.0.3 restack install writes. Load path:
`xai-grok-bundle` extract + sanitize, then `~/.grok/bundled/skills`.

**Allowlisted intercept CLI stubs** (product may keep; bash tool intercepts to Rust):

- `implement/scripts/memory.py`
- `execute-plan/scripts/validate-plan.py`
- `shared/resume-session/session_reader.py`

**Allowlisted office / PDF** (pre-reviewed exception):

- `docx/scripts/*.py` (25 files, including office helpers)
- `pptx/scripts/*.py` (26 files)
- `pdf/scripts/*.py` (8 files)
- Bundled `xlsx/` has no `.py` in this cache

**Restack regression (non-excepted):**

- `implement/tests/test_memory.py`

Bundled `implement` / `execute-plan` `SKILL.md` still tell agents to run
`python3` and do not mention the intercept. Host overlay already had the
intercept note. Product failed to defend the pin, so restack refilled the
cache.

### Host overlay `~/.agents/skills` (77 `.py`)

Operator-owned. Not rewritten.

- Same three intercept stubs (allowlisted).
- Office/PDF scripts: docx 25, pptx 26, xlsx 13, pdf 8 (allowlisted).
- `implement/tests/test_memory.py` (host copy of the same non-excepted test).
- `resume-claude/scripts/cc_session.py` (legacy host helper, not product-shipped).

Host `implement/SKILL.md` already says Grok product intercepts those CLI forms.

### Rust intercept (not dropped)

Still wired in `crates/codegen/xai-grok-tools/src/implementations/grok_build/bash/mod.rs`
around lines 2015-2031: `implement_memory`, `plan_validate`, `session_reader`.
Those CLI forms never spawn Python. Named tests still exist and pass.

## What was restored

Smallest product fix: refuse non-excepted `.py` at bundle sanitize (covers
extract, write, and prune of previously managed junk on the next sync).

- `crates/codegen/xai-grok-bundle/src/lib.rs`: `is_allowed_product_skill_python`
  plus reject in `sanitize_relative_path`. Allow only the three intercept
  stubs and `skills/{docx,pptx,xlsx,pdf}/**/*.py`.
- Named tests: `sanitize_rejects_non_excepted_skill_python`,
  `extract_archive_skips_non_excepted_skill_python`,
  `product_repo_skill_roots_have_no_non_excepted_python`.
- User-guide `08-skills.md`: section **Skills are not a Python runtime**.
  Create-skill and best-practice 5 say do not add `.py` helpers.
- `crates/codegen/xai-grok-pager/src/docs.rs`:
  `user_guide_skills_are_not_a_python_runtime`.
- Land class 7 in `FORK.md`, `AGENTS.md` Survive-recon, 
  `doc/dev/upstream-regression-filters.md`, `docs/upstream-history.md`.
- `scripts/assert-process-pins.sh`: worktree sniffs FORK "non-excepted Python",
  `08-skills.md` "not a Python runtime", and any non-excepted `.py` under
  project `.agents/skills` or `.grok/skills`.

Intercepts were not missing. No second skills home. No new Python written.

## How land will fail if this drops again

Seven land inventory classes. Class 7: product skills are not a Python runtime.

| Check | Fail condition |
|-------|----------------|
| `xai-grok-bundle` `sanitize_rejects_non_excepted_skill_python` | Sanitize accepts junk `.py` (e.g. `skills/review/scripts/build_pending_review.py`) |
| `xai-grok-bundle` `extract_archive_skips_non_excepted_skill_python` | Extract writes `implement/tests/test_memory.py` into the cache |
| `xai-grok-bundle` `product_repo_skill_roots_have_no_non_excepted_python` | Project skill roots grow junk `.py` |
| `xai-grok-pager` `user_guide_skills_are_not_a_python_runtime` | `08-skills.md` loses "not a Python runtime" |
| `xai-grok-tools` intercept tests | `memory.py` / `validate-plan.py` / `session_reader.py` spawn a shell/Python |
| `scripts/assert-process-pins.sh` | FORK loses "non-excepted Python", guide loses the sentence, or a project skill root contains junk `.py` |

Helper-green lie named in the catalog: "bundle still has `memory.py`" is not
proof the intercept still runs. Keep the tools intercept tests.

## Test / assert + red/green honesty

**Observed red (before product filter):**

```text
sanitize_rejects_non_excepted_skill_python
  accepted skills/review/scripts/build_pending_review.py
extract_archive_skips_non_excepted_skill_python
  wrote implement/tests/test_memory.py into the dest cache
```

That was the real restack hole: any nested skill `.py` was treated as a
normal bundle file.

**Same filters green after the allowlist:**

```text
CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-skills-nopy-target
TMPDIR=/home/hunter/.cache/grok-oss-tmp

cargo test -p xai-grok-bundle --lib -- \
  sanitize_rejects_non_excepted_skill_python \
  extract_archive_skips_non_excepted_skill_python \
  product_repo_skill_roots_have_no_non_excepted_python
# exit 0 (after red on the first two)

cargo clippy -p xai-grok-bundle --all-targets -- -D warnings
# exit 0 (first clippy failed on Path::canonicalize; dropped canonicalize)

cargo test -p xai-grok-tools --lib -- \
  implement_memory_snapshot_intercept_does_not_spawn_shell \
  plan_validate_intercept_does_not_spawn_shell \
  session_reader_list_intercept_does_not_spawn_shell
# exit 0, ~2m19s

cargo test -p xai-grok-pager --lib -- user_guide_skills_are_not_a_python_runtime
# exit 0 after compile (~4m13s). Earlier two runs timed out at compile.
# test docs::tests::user_guide_skills_are_not_a_python_runtime ... ok

./scripts/assert-process-pins.sh
# exit 0
# WARN only: AGENTS.md missing "parent is coordinator" (pre-existing; grep
# finds no such phrase). Not introduced by this work.
```

Did not weaken rust-centric intercept tests. Did not change their expectations.

## Leftovers

- **Stale network cache.** `~/.grok/bundled/skills/implement/tests/test_memory.py`
  is still on disk. Next launch/sync that extracts the bundle will skip it
  (and prune it if it is still a previously managed path). This report did
  not delete operator cache files.
- **Bundled SKILL.md copy.** Network `implement` / `execute-plan` still say
  run `python3` with no intercept note until the next bundle that ships
  corrected markdown. Host overlay already shadows those skills at User tier.
- **Host overlay.** Still has `implement/tests/test_memory.py` and
  `resume-claude/scripts/cc_session.py`. Operator-owned. Out of scope unless
  product starts overwriting that tree.
- **Host xlsx Python.** Host has 13 `xlsx` scripts; this bundled cache has
  none. That is overlay, not a product ship from this tree.
- **Pager clippy.** Not re-run after the docs-only test add. Lib test compiled
  and passed. Bundle clippy `-D warnings` is green.
- **No project skill tree** was created. That is correct.

## Commands + exit codes (this continuation)

| Command | Exit |
|---------|------|
| `cargo test -p xai-grok-pager --lib -- user_guide_skills_are_not_a_python_runtime` | 0 |
| Inventory `find` of product / bundled / host `.py` | 0 |

Earlier wave (same isolated dirs): bundle red then green, bundle clippy 0,
tools intercept tests 0, `assert-process-pins.sh` 0 with the pre-existing
AGENTS warn.

Stop. Parent HITL owns next launch/sync of the bundled cache.
