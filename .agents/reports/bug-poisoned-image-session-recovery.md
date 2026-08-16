# Poisoned image session recovery

The named contract is same-turn strip-retry after the server answers HTTP 400 with `code: invalid_image` on a client-valid image already in the session. Red was the same cargo test at exit 101: the fixture saw 0 Chat Completions requests because `session/load` remapped the seeded `test-model` onto grok-4.5 Responses. Green is the same command at exit 0 after load honors the seeded `test-model` and keeps it on Chat Completions. The product change lives in `keep_unverified_persisted_model` (`resolution.rs`), `restore_persisted_model` (`session_setup.rs`), `resolve_sampling_config_for_model` (`agent_ops.rs`), and `model_entry_for_apply` (`model_switch.rs`). The named test was not rewritten. The pager was not touched. grok-4.5 still uses Responses.

Status: GREEN. The named test now passes after a product fix. The test file was not rewritten.

## Red line

- Test name: `xai-grok-shell::test_image_strip_recovery::poisoned_image_session_recovers_within_the_failing_turn`
- Command (before any product edit):

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-shell --test test_image_strip_recovery poisoned_image_session_recovers_within_the_failing_turn -- --nocapture
```

- Exit code: **101**
- Fail site: `crates/codegen/xai-grok-shell/tests/test_image_strip_recovery.rs:115`
- Exact panic / assert text:

```
thread 'poisoned_image_session_recovers_within_the_failing_turn' panicked at crates/codegen/xai-grok-shell/tests/test_image_strip_recovery.rs:115:9:
expected the rejected attempt plus a strip-retry, saw 0 request(s)
```

- Observed requests: `GET /v1/models`, then `POST /v1/responses` as `grok-4.5` (small side call, then a ~46k main turn). Zero `POST /v1/chat/completions`. The scripted one-shot 400 lived on Chat Completions, so it was never consumed.

```
REQ GET /v1/models model=None keys=None body_len=0
REQ POST /v1/responses model=Some(String("grok-4.5")) keys=Some(["include", "input", "max_output_tokens", "model", "reasoning", "store", "stream", "temperature", "tool_choice", "tools"]) body_len=1172
REQ POST /v1/responses model=Some(String("grok-4.5")) keys=Some(["include", "input", "model", "prompt_cache_key", "reasoning", "store", "stream", "tools"]) body_len=46145
```

## Named contract

A session can already contain a client-valid image (a real 32x32 PNG data URI, above the load-time dimension floor) that the server still rejects with HTTP 400 and `code: invalid_image`.

Recovery must happen inside the same failing user turn, not on the next prompt:

1. After `session/load` of that history, the next user prompt must first send the poisoned image to inference.
2. The server answers 400 `invalid_image` once. The same turn must strip-retry: a second inference request that does not resend the image bytes, and that does carry a `[image removed` placeholder.
3. That strip must persist so the following turn succeeds on its first attempt (no second strip-retry cycle) and does not resend the image.
4. On disk, `chat_history.jsonl` must drop the image bytes and keep the strip placeholder.

The fixture is deliberately sendable at load. Only the server 400 is allowed to trigger the strip.

The intended live path for the seeded session model is Chat Completions (`test-model`), not remapping onto bundled grok-4.5 Responses.

## Product change

The in-turn strip-retry (sampler `RetryWithImageStrip`) and shell persist (`apply_pending_image_strip` on `Completed`) were already wired. The test never reached them because `session/load` remapped the seeded `test-model` onto grok-4.5 Responses.

Files and functions:

- `crates/codegen/xai-grok-shell/src/agent/models/resolution.rs` — `keep_unverified_persisted_model`: a persisted slug that is not in the catalog and is not `grok-*` stays as-is. Vanished `grok-*` slugs still remap within family.
- `crates/codegen/xai-grok-shell/src/agent/mvp_agent/session_setup.rs` — `restore_persisted_model`: wait for the first catalog; treat bundled defaults as not authoritative (`has_fetched_real_catalog` or remote fetch disabled); keep seeded custom slugs instead of remapping them onto grok-4.5.
- `crates/codegen/xai-grok-shell/src/agent/mvp_agent/agent_ops.rs` — `resolve_sampling_config_for_model`: unknown ids use `ModelEntry::fallback` (requested slug + Chat Completions), not a clone of the global grok-4.5 Responses config.
- `crates/codegen/xai-grok-shell/src/agent/handlers/model_switch.rs` — `model_entry_for_apply`: load/apply of a seeded custom slug uses the same fallback so `set_session_model` does not fail closed. Vanished `grok-*` still error so load can remap within family.
- `crates/codegen/xai-grok-shell/src/agent/models.rs` — `has_fetched_real_catalog` is `pub(crate)` so load can tell bundled defaults from a real fetch.
- `crates/codegen/xai-grok-shell/src/agent/mvp_agent/mod.rs` — import `keep_unverified_persisted_model`.
- `crates/codegen/xai-grok-shell/src/agent/models/tests.rs` — unit test `keep_unverified_persisted_model_keeps_seeded_custom_slug` (this helper only; older billing fixtures later in the same file were not rewritten).
- `crates/codegen/xai-grok-shell/src/agent/mvp_agent/tests.rs` — unit test `seeded_test_model_keeps_chat_completions_backend`.

Why: the named test seeds `test-model` and scripts a 400 on `/v1/chat/completions`. Honoring that seeded slug is what lets the existing strip-retry and persist path run. grok-4.5 in the catalog still uses Responses.

Not changed: the named integration test (enqueue path, helper filter, asserts), pager crate, sampler strip-retry, shell persist bodies. No git add/commit/push.

## Green line

Same command, after the product fix:

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-shell --test test_image_strip_recovery poisoned_image_session_recovers_within_the_failing_turn -- --nocapture
```

- Exit code: **0**
- Filter: 1 test ran, 1 passed
- Finished in 1.43s (post-compaction re-verify; earlier greens were 1.28s / 1.41s)
- Request log:

```
REQ GET /v1/models model=None keys=None body_len=0
REQ POST /v1/responses model=Some(String("grok-4.5")) keys=Some(["include", "input", "max_output_tokens", "model", "reasoning", "store", "stream", "temperature", "tool_choice", "tools"]) body_len=1172
REQ POST /v1/chat/completions model=Some(String("test-model")) keys=Some(["model", "messages", "tools", "stream", "stream_options"]) body_len=46294
REQ POST /v1/chat/completions model=Some(String("test-model")) keys=Some(["model", "messages", "tools", "stream", "stream_options"]) body_len=45990
```

The 1172-byte `/v1/responses` grok-4.5 call is a side path (not counted). The main turn hits `/v1/chat/completions` as `test-model` twice: first body carries the image, second is the strip-retry. Persist asserts passed.

## fmt / clippy notes

- `cargo fmt -p xai-grok-shell` exit 0.
- `cargo clippy -p xai-grok-shell -- -D warnings` (lib) exit 0. Product code in the files above is clean.
- `cargo clippy -p xai-grok-shell --all-targets -- -D warnings` exit 101. Twenty-two lib-test lints, none in the product functions this slice changed. Hits were older billing fixtures in `models/tests.rs` (lines 2334+), `subagent/tests/mod.rs`, `auth/config.rs`, `auth/manager_tests.rs`, `auth/xai_management.rs`, `replay_buffer_send_update_tests.rs`, `assistant_ascii_scrub.rs`, `shared_http_rate_limit.rs`, and `util/subprocess.rs`. This slice did not introduce those lints and did not rewrite them.

## New fork seam

New fork seam: no

This is not a new persist or sampler seam. Image-strip persist and `RetryWithImageStrip` already existed. The change is session-load / sampling / apply so a seeded custom slug stays on Chat Completions long enough for that existing recovery to run.

## Process mop

Closer ran these after the implementer report (RED and GREEN already on disk). Existing Red line, Green line, and Product change sections were left in place.

- `cargo fmt -p xai-grok-shell`: exit 0
- `cargo clippy -p xai-grok-shell --lib -- -D warnings`: exit 0
- `cargo test -p xai-grok-shell --test test_image_strip_recovery poisoned_image_session_recovers_within_the_failing_turn -- --nocapture`: exit 0 (1 passed, 0 failed; finished in 1.41s)

Post-compaction L3 re-verify (same command, no `--exact`):

- named test exit 0, 1 passed, finished in 1.43s
- `cargo fmt -p xai-grok-shell` exit 0
- `cargo clippy -p xai-grok-shell -- -D warnings` exit 0
- `cargo clippy -p xai-grok-shell --all-targets -- -D warnings` exit 101 on the same unrelated lib-test lints as above

Named test is 0. Green recorded.
