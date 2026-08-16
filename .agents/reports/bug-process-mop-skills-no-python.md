# Process mop: skills are not a Python runtime

Backup mop only. The primary implementer already ran format, clippy, and the named tests. This pass re-ran those checks on the same slice and mopped nothing.

Environment:

- `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-mop-skills-target`
- `TMPDIR=/home/hunter/.cache/grok-oss-tmp`
- rustc 1.97.1
- Did not use `/tmp`

## Commands and exit codes

| Step | Command | Exit |
|------|---------|------|
| Format | `cargo fmt -p xai-grok-bundle -p xai-grok-tools` | 0 |
| Format check (confirm no leftover dirty files) | `cargo fmt -p xai-grok-bundle -p xai-grok-tools -- --check` | 0 |
| Clippy bundle | `cargo clippy -p xai-grok-bundle --offline --all-targets -- -D warnings` | 0 |
| Clippy tools | `cargo clippy -p xai-grok-tools --offline --lib -- -D warnings` | 0 |
| Process pins | `./scripts/assert-process-pins.sh` | 0 |
| Bundle tests | `cargo test -p xai-grok-bundle --offline --lib -- sanitize_rejects_non_excepted_skill_python extract_archive_skips_non_excepted_skill_python product_repo_skill_roots_have_no_non_excepted_python` | 0 |
| Tools tests | `cargo test -p xai-grok-tools --offline --lib -- implement_memory_snapshot_intercept_does_not_spawn_shell plan_validate_intercept_does_not_spawn_shell session_reader_list_intercept_does_not_spawn_shell` | 0 |
| Pager docs test | `cargo test -p xai-grok-pager --offline --lib -- user_guide_skills_are_not_a_python_runtime` | 0 |

Clippy on `xai-grok-tools` stayed `--lib`. `--all-targets` was not needed; `--lib` finished clean.

`assert-process-pins.sh` printed a warning that `AGENTS.md` is present but missing the expected `parent is coordinator` pin. The script still exited 0: all required process-pin paths are present (24 files + 5 dirs). That warning is not fallout from this slice. This mop did not edit `AGENTS.md`.

Did not run `cargo fmt -p xai-grok-pager`. A sibling mop owns pager format.

## Named tests (all green)

`xai-grok-bundle` (`--lib`): 3 passed, 0 failed, 43 filtered out.

- `tests::sanitize_rejects_non_excepted_skill_python`
- `tests::extract_archive_skips_non_excepted_skill_python`
- `tests::product_repo_skill_roots_have_no_non_excepted_python`

`xai-grok-tools` (`--lib`): 3 passed, 0 failed, 3198 filtered out.

- `implementations::grok_build::bash::tests::implement_memory_snapshot_intercept_does_not_spawn_shell`
- `implementations::grok_build::bash::tests::plan_validate_intercept_does_not_spawn_shell`
- `implementations::grok_build::bash::tests::session_reader_list_intercept_does_not_spawn_shell`

`xai-grok-pager` (`--lib`): 1 passed, 0 failed, 8887 filtered out.

- `docs::tests::user_guide_skills_are_not_a_python_runtime`

## Edits

This mop edited nothing. No product files, no tests, no host overlay under `~/.agents/skills`, and no new Python.

## Leftover host cache

Still present, and still expected until the next product launch or bundle sync prune:

`~/.grok/bundled/skills/implement/tests/test_memory.py`

That file is leftover host cache. It is not a product skill source file in this repo. Do not treat it as a failed land of the in-repo "skills are not a Python runtime" contract.

## Result

Green mop. No fallout from this slice.
