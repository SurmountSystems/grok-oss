# Review: pager cluster, image-strip restore, FORK leftovers, lock-takeover flake

Independent L3 review. This reviewer did not implement the work and did not edit product. Spot-checked the named files. Re-ran the four pager cluster tests plus the six keep-green paint-and-click filters, the named image integration test plus the two new restore unit tests, and the lock-takeover flake test. Env: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`.

## Verdict

**Approve with nits.**

They did not fit the named failing tests to broken code. Pager always-on bubble copy is still paint plus click. Wrap columns, table detect, and selectable line identity stay honest. The image fix honors the seeded `test-model` on Chat Completions instead of rewriting the integration test. Vanished `grok-*` remapping is still in place. grok-4.5 still uses Responses. SuperGrok is paid. Docs leftovers 1 through 8 are on disk. UNPROVEN seams stay labeled. The new restore seam is extras only, enrolled with three proven `fn`s. The lock-takeover fixture wait does not change the lock contract.

No remaining contract bug in this slice needs a new failing test.

## Pager

**Did they fit tests to broken code?** No.

The four cluster tests still encode the same contracts the red report named. They still use default appearance (`bubble_copy_buttons: true`). `make_markdown_entry` is still `RenderBlock::agent_message`. `render_with_scratch` still uses `AppearanceConfig::default()`. None of the four turn the glyph off, change expected line counts, or loosen table detect.

| Test | Still asserts |
|------|----------------|
| `table_copy_uses_width_snapshot_when_anchor_block_scrolled_out` | Detectable grid at snapshot width 40, then cell copy |
| `test_selection_model_top_clipped_markdown_entry` | First selectable line at `screen_y == 0` after scroll 1 |
| `overlay_pretty_link_url_with_cjk_text` | Combined OSC 8 fragment widths equal URL display width |
| `message_block_content_width_subtracts_timestamp_reservation` | Model line count equals `effective_output` at the reserved wrap width |

Helper tests in `blocks/mod.rs` and `blocks/user.rs` were updated to the intended contract (no inserted chrome line, no span-width change, paint at the hit column). That is a stronger check, not a rewrite of the four red tests.

**Is bubble copy still paint plus click?** Yes.

`append_bubble_copy_button` only writes `copy_button_col` on line 0. It does not append `" "` + `⧉` and does not insert a `BlockLine`. Slack: hit column is `used + 1`. Slack gone: hit column is `ctx.content_width()` (first timestamp-gutter or right-pad cell). `BlockLine::paint_bubble_copy_button` paints after content and the timestamp overlay in `EntryRenderer::render` and the sticky-header path in `scrollback_pane.rs`. Hit-testing still uses `bubble_copy_button_rect` / `copy_button_col`. Agent and user output still call the helper. Default `bubble_copy_buttons` is still `true`.

Keep-green filters still require a painted `copy_icon()` and a click that copies payload without the glyph:

- `append_bubble_copy_button_paints_when_first_line_fills_content_width`
- `bubble_copy_buttons_on_paints_copy_icon`
- `bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width`
- `clicking_wide_human_bubble_copy_still_paints_and_copies`
- `clicking_assistant_bubble_copy_copies_the_message`
- `clicking_human_bubble_copy_copies_the_prompt`

**Independent re-run:** those ten pager filters, 10 passed, 0 failed.

**Wrap / table / selection identity.** Honest. Content spans and `output().lines` length no longer change when the glyph is on. That is why table detect no longer sees a `selection_range == None` hole, why CJK overlay width is 68 not 67, and why the timestamp proxy is 5 vs 5 instead of 6 vs 5.

Nit (not a failed land): when the first wrap line is already full, `⧉` paints in the first reserved gutter cell and can overwrite the timestamp's leading space. That is the intended pad/gutter paint, not wrap-content mutation.

## Image-strip

**Did they fit the named test to broken code?** No.

`tests/test_image_strip_recovery.rs` still seeds `test-model`, still scripts one HTTP 400 `invalid_image` on `/v1/chat/completions`, and still requires a same-turn strip-retry plus persist. The fixture was not rewritten.

**Is the product fix (honor seeded model) correct vs rewriting the test?** Yes.

The in-turn strip (`RetryWithImageStrip`) and persist (`apply_pending_image_strip`) were already wired. Red was `session/load` remapping `test-model` onto grok-4.5 Responses, so the scripted 400 was never consumed (0 Chat Completions). The product change is restore / sampling / apply:

- `keep_unverified_persisted_model`: not in catalog and not `grok-*` stays as-is.
- `restore_persisted_model`: waits for the first catalog; bundled defaults are not authoritative; seeded custom slugs are kept.
- `resolve_sampling_config_for_model`: unknown ids use `ModelEntry::fallback` (requested slug + `ApiBackend::default()` = Chat Completions).
- `model_entry_for_apply`: same fallback for seeded custom slugs.

`ModelInfo::fallback` uses `ApiBackend::default()`, which is Chat Completions. Bundled `default_models.json` still has grok-4.5 `"api_backend": "responses"`.

**Is remapping of vanished `grok-*` still intact?** Yes.

`keep_unverified_persisted_model` is false for `grok-*`. `restore_persisted_model` still takes `same_family_fallback`. `model_entry_for_apply` still errors on vanished `grok-*` so load remaps instead of applying Chat Completions fallback. New unit tests pin both sides: `test-model` kept; `grok-4.3` and live `grok-4.5` not treated as unverified; vanished `grok-4.3-not-in-this-catalog` fails apply.

**Does grok-4.5 still use Responses?** Yes. Catalog file plus apply comments. The integration test may still make a small grok-4.5 `/v1/responses` side call. That is not the main turn. The main turn is Chat Completions as `test-model`.

**Independent re-run:** `poisoned_image_session_recovers_within_the_failing_turn` passed (1.33s). `keep_unverified_persisted_model_keeps_seeded_custom_slug` and `seeded_test_model_keeps_chat_completions_backend` passed.

The implementer said this is not a new numbered land class. That is right. Enrolling it as extras (below) is still correct.

## Docs

**Leftovers 1 through 8:** done on disk.

| Item | Spot-check |
|------|------------|
| 1. `03-keyboard-shortcuts.md` | Plan keys `a` / `A` / `?` / `s` / `q`. Empty Enter never approves. Footer Enter cue is send / queue / interject. Soft interject never cancels. Cancel is Esc / `[stop]`. Old "Send now (cancels the current turn)" wording is gone from this page. |
| 2. `16-subagents.md` | Mid-turn interject injects and never cancels. Points at the keyboard page. |
| 3. `22-permissions-and-safety.md` | Always-approve skips tool-permission prompts only. It does not click plan Approve. Links `19-plan-mode.md`. |
| 4. FORK class 5 | `# 5.` is hop + after-burner + Business / Team + flock + combined remaining + 5b compact-meter names. `show_limits`, `format_supergrok_session`, `footer_names_live_principal`, and the two `limits_json_*` names are in the neighbor cargo block. |
| 5. Dead identifiers | Same-batch bullet names `same_batch_plan_write_before_exit_plan_mode_returns_new_body`. Soft-interject bullet drops `enter_prompt_mode` and labels the footer cue as shipped with no named footer `fn`. Dogfood snapshot cargo no longer lists the no-`fn` names. |
| 6. Catalog extra honesty | Extra `from_config` says empty `models_cache.json` is a code miss with no named test. Extra Nucleo says `NUM_NUCLEO_THREADS = 2` is shipped and no `fn` asserts `Some(2)`. |
| 7. Operator class 3 heading | `# 3. Token Economy ledger /spend (extra SQL, not SuperGrok dollar credits)`. Required land heading is still `### 3. grok-oss SQL extras`. |
| 8. Language | New pin sentences are ASCII. No "free SuperGrok." No "out-of-allowance mark." No media-player pause metaphor. |

**UNPROVEN still labeled.** rustc 1.97.1 file pin only. Empty `models_cache.json` not cargo-proven. Nucleo `Some(2)` not cargo-proven. Stuck-retry pager chrome names still forbidden. Title-item ghosts still forbidden. Lower-left throbber color still absent. Token Economy `/settings` rows not re-proven. Host `~/.agents/skills` is not a land class. Live TUI dogfood still operator-gated. Footer Enter cue still "shipped in code, no named footer `fn`."

**New seam enrolled only with proven fns.** Yes. FORK Product extras + extra restack-droppable class + cheat sheet, and catalog extra section + operator extra cargo, list only:

- `keep_unverified_persisted_model_keeps_seeded_custom_slug`
- `seeded_test_model_keeps_chat_completions_backend`
- `poisoned_image_session_recovers_within_the_failing_turn`

Those `fn`s exist. Product function names (`keep_unverified_persisted_model`, `restore_persisted_model`, …) are not listed as cargo land. This is not a new seventh or eighth numbered land class. It is not last-session on start.

Nit: `.agents/reports/fork-docs-image-seam-pin.md` says the implementer marked "New fork seam: yes." The implementer report says "New fork seam: no." The pin's enrollment (extras, proven `fn`s only) is still the right docs call.

Nit (not leftover 1): `crates/codegen/xai-grok-pager/src/actions/defaults.rs` `ActionId::InterjectPrompt` still has description "Send now while running (cancels the current turn)". The user-guide leftover is fixed. That palette string is stale product copy relative to FORK / `interject_contract_*`. Out of this docs-finish write.

## Flake

**Lock contract unchanged.** Yes.

`take_over_declines_when_lock_is_never_released` still requires: predecessor named in the pidfile, in-process flock held, predecessor terminated, `Ok(None)`, pidfile not rewritten. Product path `acquire_or_take_over_matching` is the same: signal, grace, kill, `poll_acquire`, then `Ok(None)` if the flock is still held. `TAKEOVER_GRACE` (2s), `TAKEOVER_KILL_GRACE` (1s), and `TAKEOVER_POLL` (50ms) are untouched.

The harden is fixture-only: `spawn_predecessor` waits until `process_name_matches(pid, "sleep")` (2s bound, 10ms poll) so `/proc/<pid>/cmdline` is not empty at handoff. That is the same identity check the product uses. It is not a longer takeover grace.

**Independent re-run:** 1 passed. Logs show SIGTERM wait, SIGKILL, decline because the lock is still held.

No local red of the named cargo test was claimed. The flake evidence is the empty-cmdline probe plus the sibling comment. That is honest. The fixture wait matches that cause.

## Suggested failing tests

None. The named contracts in this slice hold. Do not add cargo for UNPROVEN seams. Do not invent a footer Enter-cue `fn` just to enroll it.

## Must-fix before mop vs nits

**Must-fix before mop:** none.

**Nits (do not block mop):**

1. Image-seam pin report mis-cites the implementer's "new fork seam" line. Disk enrollment is still correct.
2. `InterjectPrompt` action description still says send-now cancels. User-guide leftover 1 is done. Separate product-copy follow-up if anyone touches that palette.
3. Full-width first line: `⧉` may overwrite the first timestamp-gutter cell. Intended paint-in-gutter.
4. `cargo clippy -p xai-grok-shell --all-targets -- -D warnings` remains 101 on older lib-test lints. The implementer recorded that. None of those hits are in `resolution.rs`, `session_setup.rs`, `agent_ops.rs`, or `model_switch.rs`. Lib clippy on that crate is clean.

End of review.
