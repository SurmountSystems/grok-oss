# Restack onto public Grok Build 1.0.3

Date: 2026-08-12. Branch: `onto-xai/b13fa526f511` (PR SurmountSystems/grok-oss#36). Shared cwd. No worktree.

## Result

| Check | Value |
|-------|--------|
| New tip SHA | `755521df88eab2da16b36e189b7ac0329e73c859` |
| Tip tree | `4134d42c1cd0b1f3ee15f65fa0a3804385fd8c98` |
| xAI ancestor `e5fd4816d43260c15ba785f103990c1ed6cea230` | yes |
| `origin/main` ancestor (`f17e84d8`) | yes |
| Cherry-pick sequencer | clean (no `CHERRY_PICK_HEAD`, no sequencer) |
| rustc | 1.97.1 (`rust-toolchain.toml` / fenix pin kept) |
| Assert `./scripts/assert-process-pins.sh HEAD` | OK (24 files + 5 dirs) |
| Push | `git push --force-with-lease origin onto-xai/b13fa526f511` succeeded: `09c407e2...755521df` (forced update). No `gh` write. |

Old joined tip remains on `backup/onto-xai-b13fa526f511-0f61ff44-joined-20260812`. Untracked `.agents/reports/` were not committed.

## Stack SHAs

| Role | SHA |
|------|-----|
| xAI 1.0.3 tip | `e5fd4816d43260c15ba785f103990c1ed6cea230` |
| First-parent onto tip (24 picks, `09c407e2` replay) | `ee8a80d28cf5df2841b3762396b5921837e15813` |
| Join (`-s ours`, two-parent commit-tree) | `e08e596167538f9e72da0760865340adfa34868f` (parents `ee8a80d2` + `f17e84d8`; join tree `ae3568e6`) |
| Compile mop | `4651593a1da1bbaf2831f316791cfb6d69c663e6` |
| Docs live-stack update (current HEAD / remote tip) | `755521df88eab2da16b36e189b7ac0329e73c859` |

Intermediates used `cherry-pick -x --no-commit` then `commit-tree` + `update-ref`. No `commit.gpgsign=false`, no `--no-gpg-sign`, no fake `gpg.program`.

## Pick count

24 first-parent commits from `b13fa526f511..09c407e2` were applied. 0 skipped as empty. `0f61ff44` (old join) was not picked. Join ran after the restack.

Applied first-parent range `e5fd4816..ee8a80d2`:

1. `b46f12d0` OpenRouter (`336a14ea`)
2. `720a8195` Brand Grok OSS (`c35534b8`)
3. `91724dde` Rate limits (`75a84a52`)
4. `b065a626` Merge 2 (`67339bf0`)
5. `f476a63e` impl #7 (`01327f98` / `4ee1ce8e`)
6. `c903170a` merge xai 2 (`f78f0a90`)
7. `cdf1282b` compaction (`e74c492a`)
8. `f1ae2ccc` merge xai 3 (`0cb12369`)
9. `598c7139` soft interject (`5026d71c`)
10. `62ac70a0` (`0ad6dc97`)
11. `6b30c61e` (`634e35c3`)
12. `10a2f5d1` 521/retry (`785bafc9` / `bf98394a`)
13. `64a85b6f` dual-auth (`5ccaea3a` / `c87f66a6`)
14. `db0e3cb5` (`a91a3060` / `0ff9fb45`)
15. `2def5f78` textarea (`37b0f543` / `3ade84f0`)
16. `2d475b2b` Cargo.toml (`26ac49ae`)
17. `0630fea7` pager/workspace features (`f1a7eaa2`)
18. `4601358e` local-workspace (`9217f653`)
19. `fffb934f` test-support (`11f4fd5c`)
20. `037c444f` docs (`9060f502`)
21. `5fef81f2` docs SHA (`2c34b6c8`)
22. `0c9596be` mop (`e60383d9`, 17 UU, kept HEAD / 1.0.3 structure)
23. `461099d5` docs (`241f6f12`, clean)
24. `ee8a80d2` merge upstream (`09c407e2`, 35 UU)

## Notable conflicts and preference

Default: keep 1.0.3 monorepo APIs (`#[path]` split tests, new crates). Re-apply Surmount product seams. Never wholesale `checkout --theirs` after 1.0.3 is HEAD.

`09c407e2` auto-merge was the main compile breaker. Incoming product dumps were older than 1.0.3 and rewound tip APIs (`session_registry`, `ShutdownKind`, request-path helpers). Restoring only the 09c-changed shell files from `461099d5` did not compile. Restoring 1.0.3 API cores from `e5fd4816` and re-applying Surmount-only modules plus small shims did.

| Area | Preference used |
|------|-----------------|
| `xai-fuzzy-file-search` | Take 1.0.3 crate. Keep `Nucleo::new(..., Some(2), 1)`. |
| `FuzzySearchManager` | Keep Surmount reuse-per-root. Poll must not write `last_activity`. |
| `handle.rs` fuzzy getters | Keep `&self`. 300s timeout kept. |
| `views/history_search.rs` | Take 1.0.3 lazy spawn on first activate. Drop does not join. |
| `xai-grok-active-sessions`, `xai-grok-session-search` | Take 1.0.3. Active-sessions `list()` uses `xai_grok_config::grok_home()`. |
| Shell / pager 09c dumps | Restore 1.0.3 cores. Keep Surmount-only auth, token_economy, grok_oss, limits, rebuild, soft-stop, notes. |
| `xai-grok-update` | Restore 1.0.3 `auto_update`. Import `xai_grok_active_sessions` (shell no longer re-exports it). |
| Plan panel | 1.0.3 three-button `plan.rs` stays. Older 6904-line five-CTA view was not wholesale-restored (see residual). |
| Tip deletions | Dead coordinator/trace files stay deleted if 1.0.3 removed them. |

## Nucleo contract

Present: yes.

- `crates/codegen/xai-fuzzy-file-search/src/lib.rs`: `const NUM_NUCLEO_THREADS: usize = 2` and `Nucleo::new(..., Some(NUM_NUCLEO_THREADS), 1)`.
- `crates/codegen/xai-grok-workspace/src/file_system/mod.rs`: reuse-per-root manager; `get_results` / `get_results_filtered` are `&self`.

Test command: `cargo test -p xai-grok-workspace --lib file_system::tests -- --test-threads=1`

Result: ok. 3 passed.

- `repeated_open_without_close_keeps_one_search_per_root`
- `distinct_roots_each_keep_one_search`
- `get_results_does_not_keep_a_stale_search_alive`

History-search construction (1.0.3 lazy spawn): `cargo test -p xai-grok-pager --lib views::history_search -- --test-threads=1`

Result: ok. 15 passed, including `construction_does_not_spawn_the_daemon`, `reactivation_reuses_the_daemon`, and `refresh_query_poll_are_noops_before_first_activation`.

## Compile mop

Shell `--lib` green. Pager `--lib` green. `xai-grok-update`, `xai-grok-sampler`, `xai-grok-active-sessions` `--lib` green. No conflict markers in those crates.

Compile-mop highlights:

- Shell Cargo.toml: restore `dhat` optional dep, `xai-workflow`, Surmount rusqlite / `xai-sqlite-journal`.
- Sampler / sampling-types: `parse_error_bytes` + `user_facing_api_error_message`; cancel uses `auth_unknown`.
- Pager: re-declared Surmount modules; StreamResumed arm; 461 `credit_bar` + `Default` for extra fields; Action / Effect / TaskResult shims for limits, spend, notes, rebuild, global pause, soft stop.
- Test-compile mop so pager `--lib` tests build: `AppView` fixture fields, `TodoItem.size`, `AutoCompactStarted` threshold fields, `BillingConfig.product_usage`, `ThemeKind::Doge`, `browser_unavailable_message` (1.0.3 one-arg API).

## Residual

Pinned in `RESIDUAL.md`:

- Plan five-CTA (Approve / Notes / Clarify / Revise / Quit, present is not approve) is not wholesale-restored. Tip has the 1.0.3 three-button placeholder.
- Dual-auth spend-order wiring in `sampling_config_for_model` may still use empty failover after the 1.0.3 AuthManager restore.
- Dogfood still needs install plus a new `grok-oss` process. SuperGrok is paid. Say included SuperGrok period limits, SuperGrok dollar credits, and console team prepaid / console API credits as distinct meters.

## Push

`git push --force-with-lease origin onto-xai/b13fa526f511` updated the existing PR branch. Remote tip is `755521df`. No new PR. No `gh pr` write.
