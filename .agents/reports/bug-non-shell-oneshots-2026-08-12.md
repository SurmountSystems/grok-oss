# Non-shell non-pager CI residual oneshots (2026-08-12)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 implementer
**Source inventory:** `.agents/reports/bug-shell-residual-inventory-2026-08-11.md` § non-shell (~7)
**Out of scope:** `xai-grok-pager`, `xai-grok-shell` product mass (other agents).

## Summary

| Package | Filter / oneshot | Live result | Fix | Remaining |
|---------|------------------|-------------|-----|-----------|
| `xai-grok-tools` | `capabilities_is_read_only_matches_metadata` | **PASS** (was red) | Override `ToolMetadata::is_read_only` on `json_to_toon` + `plan_validate` | none |
| `xai-grok-tools` | `non_pi_finalized_contract_snapshot_is_unchanged` | **PASS** (was red) | Refresh contract snapshot to current `todo_write` fib/meta/size wire | none |
| `xai-grok-agent` | `test_encrypted_templates_not_stale` | **PASS** (was red) | Regenerated `prompt_encrypted.rs` via crate `scripts/encrypt_templates.py` | none |
| `xai-grok-hooks` | `hook_child_cannot_open_dev_tty` | **PASS** (already green) | none | none |
| `xai-grok-pager-minimal` | `committed_thinking_paints_a_dim_rail_in_column_zero` | **compile fail** | blocked | needs pager APIs (see below) |
| `xai-grok-sampler` | `status_user_message_matrix` (`cf_edge_error_message`) | **PASS** (was red) | Finer CF status copy in `status_user_message` (525/526 SSL, 529 overloaded) | none |
| `xai-grok-update` | `install_internal_from_bases_does_not_fallback_on_smoke_failure` | **PASS** (was red) | Smoke fail aborts multi-base loop (no CDN hop) | none |
| `xai-grok-pager-render` | auto dark → DOGE (`resolve_auto_dark_*`, `resolve_from_config_auto_sets_auto_mode_dark`) | **PASS** (was red) | `to_theme_kind` dark default `GrokNight` → `Doge` | none |

**Working residual after this wave:** **1 package blocked** (`xai-grok-pager-minimal`), **0** greenable non-shell oneshots left from the inventory list.

---

## Per package

### 1. `xai-grok-tools` (2)

**Red:**
- `registry::types::tests::capabilities_is_read_only_matches_metadata` — `GrokBuild:json_to_toon` and `GrokBuild:plan_validate` had `ToolMetadata::is_read_only()=false` (kind `Other`) vs `capabilities().is_read_only=true`.
- `registry::types::tests::non_pi_finalized_contract_snapshot_is_unchanged` — snapshot lagged intentional `todo_write` board copy (fib size, meta, protected-prefix merge).

**Fix:**
- `json_to_toon/mod.rs`, `plan_validate/mod.rs`: override `fn is_read_only(&self) -> bool { true }` (same pattern as `get_task_output`).
- Snapshot in `registry/types.rs` refreshed to product wire (not a weakened assert; documents current contract).

**Verify:** both filters **ok**.

### 2. `xai-grok-agent` (1)

**Red:** `prompt::template::tests::test_encrypted_templates_not_stale` — `prompt.md` plaintext ahead of `prompt_encrypted.rs`.

**Fix:** ran existing `crates/codegen/xai-grok-agent/scripts/encrypt_templates.py` → rewrote `src/prompt/prompt_encrypted.rs`.

**Verify:** filter **ok**.

### 3. `xai-grok-hooks` (1)

**Live:** `runner::command::tests::test_hook_child_cannot_open_dev_tty` **ok** (env-ish; green on this host). No edit.

### 4. `xai-grok-pager-minimal` (1) — **REMAINING**

**Cannot run** dim-rail filter: package **does not compile** against current `xai-grok-pager`:

| Missing API (in `xai-grok-pager`) | Call site (pager-minimal) |
|----------------------------------|---------------------------|
| `EntryRenderer::with_dim_accent` | `commit.rs` `minimal_renderer` |
| `ScrollbackState::insert_block_before` | `plan.rs` + plan commit tests |

Out of scope for this worker: editing `xai-grok-pager`. Dim-rail contract wants `Modifier::DIM` on column-0 thinking rails; that needs `with_dim_accent` (or equivalent) on pager’s entry renderer. Plan pre-tool insert needs `insert_block_before` on scrollback state.

**Handoff for pager owner:** implement those two public APIs (or restore if half-merge lost them), then green `committed_thinking_paints_a_dim_rail_in_column_zero` and plan insert tests in pager-minimal.

### 5. `xai-grok-sampler` (1)

**Red:** `status_user_message_matrix` — 525/526 expected `"Secure connection"`, product lumped them into generic timeout; 529 expected `"overloaded"`.

**Fix:** `xai-grok-sampling-types/src/error.rs` `status_user_message`:
- 525–526 → Secure connection copy
- 529 → overloaded copy
- keep 521 origin-down; other 52x / 530 timed-out
Citation note in code: Cloudflare 5xx docs (accessed: 2026-08-12).

**Verify:** `cargo test -p xai-grok-sampler --test cf_edge_error_message status_user_message_matrix` **ok**.

### 6. `xai-grok-update` (1)

**Red:** `install_internal_from_bases_does_not_fallback_on_smoke_failure` — primary served non-executable smoke-failing artifact; multi-base loop retried fallback → `Download failed: HTTP 404`.

**Fix:** `install_internal_from_bases` returns immediately on smoke-shaped errors (`failed to run` / `exited`); only transport/resolution failures hop to the next base.

**Verify:** full `test_install_internal` **19/19 ok**.

### 7. `xai-grok-pager-render` (2, re-check)

**Red:** `resolve_auto_dark_system_returns_doge`, `resolve_from_config_auto_sets_auto_mode_dark` — auto dark mapped to `GrokNight` while product default / tests demand **DOGE**.

**Fix:** `to_theme_kind` dark default `ThemeKind::Doge`; align unit tests (`to_theme_kind_dark_defaults_to_doge`, light-override ignore).

**Verify:** auto-dark DOGE filters + `to_theme_kind_dark_*` **ok**.

---

## Commands used (nice)

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-tools --lib capabilities_is_read_only_matches_metadata
nice -n 19 ionice -c3 cargo test -p xai-grok-tools --lib non_pi_finalized_contract_snapshot_is_unchanged
nice -n 19 ionice -c3 cargo test -p xai-grok-agent --lib test_encrypted_templates_not_stale
nice -n 19 ionice -c3 cargo test -p xai-grok-hooks --lib hook_child_cannot_open_dev_tty
nice -n 19 ionice -c3 cargo test -p xai-grok-sampler --test cf_edge_error_message status_user_message_matrix
nice -n 19 ionice -c3 cargo test -p xai-grok-update --test test_install_internal
nice -n 19 ionice -c3 cargo test -p xai-grok-pager-render --lib resolve_auto_dark_system_returns_doge
nice -n 19 ionice -c3 cargo test -p xai-grok-pager-render --lib resolve_from_config_auto_sets_auto_mode_dark
cargo fmt -p xai-grok-tools -p xai-grok-agent -p xai-grok-sampling-types -p xai-grok-update -p xai-grok-pager-render
# agent encrypt:
(cd crates/codegen/xai-grok-agent && python3 scripts/encrypt_templates.py)
```

## Files touched

| Path | Why |
|------|-----|
| `crates/codegen/xai-grok-tools/src/implementations/grok_build/json_to_toon/mod.rs` | is_read_only override |
| `crates/codegen/xai-grok-tools/src/implementations/grok_build/plan_validate/mod.rs` | is_read_only override |
| `crates/codegen/xai-grok-tools/src/registry/types.rs` | contract snapshot |
| `crates/codegen/xai-grok-agent/src/prompt/prompt_encrypted.rs` | regenerated encrypt |
| `crates/codegen/xai-grok-sampling-types/src/error.rs` | CF status copy |
| `crates/codegen/xai-grok-update/src/auto_update.rs` | smoke no multi-base hop |
| `crates/codegen/xai-grok-pager-render/src/theme/system_appearance.rs` | auto dark → DOGE |

No git commit/add/push.

## 5-line bottom line

1. **6/7 packages green** after product fixes; inventory oneshots largely closed.
2. Tools: metadata/caps read-only agreement + todo_write snapshot.
3. Agent templates re-encrypted; hooks already green.
4. Sampler CF matrix, update smoke-no-fallback, pager-render auto dark DOGE fixed.
5. **pager-minimal remains red/compile-blocked** on pager-owned `with_dim_accent` + `insert_block_before`.
