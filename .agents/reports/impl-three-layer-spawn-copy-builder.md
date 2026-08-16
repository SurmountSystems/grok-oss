# Report: spawn-tool copy teaches always three layers

## Files changed

- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-agent/src/builder.rs`
  - rustdoc on `CHILD_TASK_DESCRIPTION`
  - `CHILD_TASK_DESCRIPTION` product string (L2 spawn-tool description override)
  - `builder::tests::child_task_description_is_concise` (same test name; stronger contract)

Not edited:

- `crates/codegen/xai-grok-agent/templates/subagent_prompt.md` (shared L2/L3; see leftovers)
- `RESIDUAL.md`

## Exact new `CHILD_TASK_DESCRIPTION` wording

Rendered string L2 sees on the spawn tool:

```
Launch a specialist (L3) for an independent sub-task that needs its own context.

You are L2. Whenever work is to be done and tools are to be called, agents are three layers deep. Always. Including implement loops. You MUST always spawn L3 for any tools/work. L2 never does greps, edits, or tests. Do not compact-and-continue a product restore on L2. No L4.

Usage: specify ${{ params.task.subagent_type }} ("general-purpose", "explore", or "plan"), a short ${{ params.task.description }}, and a detailed ${{ params.task.prompt }}.
${{ params.task.run_in_background }}: Returns immediately with a subagent_id. Use the task output tool to retrieve results. This is set to true by default.
```

Doc comment now says L2 always spawns L3 for any tools/work, L2 never does greps/edits/tests, three layers always.

## Test names changed/added

- Same family, same name: `builder::tests::child_task_description_is_concise`
- No new test name
- Strengthened (not weakened): now requires
  - `three layers deep. Always`
  - `MUST always spawn L3 for any tools/work`
  - `L2 never does greps, edits, or tests`
  - `Including implement loops`
  - still forbids compact-and-continue, `Agent types:`, `<example>`
  - still `len() < 700`
  - **rejects** old wording: `many greps`, `half the window`

## Whether tests now assert always-three-layer

Yes. The same test now asserts three layers always, always spawn L3 for any tools/work, L2 never greps/edits/tests, implement loops included, and that the old many-greps / half-window rule is absent.

## Whether the old test / wording was observed first

Yes.

1. Ran `child_task_description_is_concise` against the old copy first. **Passed** (exit 0). Old test only required `MUST spawn L3` plus compact-and-continue / length. It would stay green on the weaker half-window rule.
2. Tightened the same test to the new contract. **Red** (exit 101): `child description must teach three layers always`.
3. Updated product copy. First draft was 727 chars and failed the existing `len() < 700` bound (exit 101). Tightened without raising that bound. **Green**.

## Commands + exit codes

Env: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-three-layer-spawn-copy-target` `TMPDIR=/home/hunter/.cache/grok-oss-tmp` rustc 1.97.1 (8bab26f4f 2026-07-14).

| Step | Command | Exit |
|------|---------|------|
| Observe old wording | `cargo test -p xai-grok-agent --lib child_task_description_is_concise -- --nocapture` | 0 (old test green on weak copy) |
| Red after stronger asserts | same test filter | 101 |
| Green after copy (first over-length) | same | 101 (`got 727 chars`) |
| Green after compact copy | same | 0 |
| fmt | `cargo fmt -p xai-grok-agent` | 0 |
| clippy | `cargo clippy -p xai-grok-agent --all-targets -- -D warnings` | 0 |
| Related module | `cargo test -p xai-grok-agent --lib builder::tests -- --nocapture` | 0 (42 passed) |

## Leftovers

- **`templates/subagent_prompt.md` is still mixed.** It is a shared worker prompt: it tells the recipient to complete the assigned task and use tools. There is no L2-vs-L3 split. Teaching "L2 never uses tools" there would also bind L3. Left alone on purpose.
- The named law also says "Regardless of perceived complexity." That phrase is **not** in `CHILD_TASK_DESCRIPTION` so the string stays under the existing 700-char compact bound. The test still requires three layers always, implement loops, always-spawn-L3, and no half-window / many-greps teaching.
- Crate search: no remaining `many greps` / `half the window` product copy in `xai-grok-agent` except the test's **negations**.
