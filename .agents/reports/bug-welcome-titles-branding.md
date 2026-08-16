# Welcome + window-title branding restore

**Date:** 2026-08-13
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Slice:** operator-visible Welcome chrome and title brand after the 1.0.3 restack.

`spawn_subagent` is not available in this L2 session (no host spawn tool). Work stayed in this window on the named welcome + title modules. SuperGrok is paid. This report says **included SuperGrok period limits**, not "free SuperGrok."

## Contract

- Welcome badge, hero subtitle, tutorial title, and pager-minimal welcome say **Grok OSS**, not Grok Build.
- `TitleItem::Grok` (config slot still named `grok`) emits **`grok-oss`**.
- Window/tab OSC fallback is **`grok-oss`**, not bare `grok`.
- Restored catalog names: `product_cli_name_is_grok_oss`, `window_title_always_manages_non_empty_branded_osc`, `window_title_osc_payload_never_empty_string`, `titles_on_session_name_osc_is_non_empty_branded`.

## TDD

Named tests landed first against leftover **Grok Build** / bare `grok` copy.

Red command (intended, before product strings changed):

```bash
cargo test -p xai-grok-pager --lib -- \
  welcome_badge_brands_grok_oss hero_subtitle_brands_grok_oss \
  tutorial_list_title_brands_grok_oss title_item_grok_emits_grok_oss \
  window_title_always_manages_non_empty_branded_osc
```

**Observed red:** not a test assertion line. Shared `target/` stayed under other pager cargo jobs when the tests were first written. Isolated compile later ran only after product strings already said Grok OSS / `grok-oss`. The runner never printed `Welcome badge must say Grok OSS` or `left: "grok" right: "grok-oss"`. Source at test-write time still painted `Grok Build` / `grok`. That is weaker than a captured assertion fail.

Green command (re-run after product restore, this turn):

```bash
cargo test -p xai-grok-pager --lib -- \
  product_cli_name_is_grok_oss title_item_grok_emits_grok_oss \
  welcome_badge_brands_grok_oss hero_subtitle_brands_grok_oss \
  tutorial_list_title_brands_grok_oss \
  window_title_always_manages \
  window_title_osc_payload_never_empty_string \
  titles_on_session_name_osc_is_non_empty_branded
```

**Green observed.** 8 passed, 0 failed. Names that ran:

- `client_identity::tests::product_cli_name_is_grok_oss`
- `notifications::title::tests::title_item_grok_emits_grok_oss`
- `views::welcome::tests::welcome_badge_brands_grok_oss`
- `views::welcome::hero_box::tests::hero_subtitle_brands_grok_oss`
- `views::tutorial::tests::tutorial_list_title_brands_grok_oss`
- `app::tests::window_title_always_manages_non_empty_branded_osc`
- `app::tests::window_title_osc_payload_never_empty_string`
- `app::tests::titles_on_session_name_osc_is_non_empty_branded`

Pager-minimal:

```bash
cargo test -p xai-grok-pager-minimal --lib -- pager_minimal_welcome_brands_grok_oss
```

**Green observed.** `welcome::tests::pager_minimal_welcome_brands_grok_oss` passed.

```bash
cargo fmt -p xai-grok-pager -p xai-grok-pager-minimal -- --check
```

**fmt exit 0.**

```bash
cargo clippy -p xai-grok-pager --lib -- -D warnings
cargo clippy -p xai-grok-pager-minimal --lib -- -D warnings
```

**Clippy `--lib` exit 0** on both packages.

`cargo clippy -p xai-grok-pager --all-targets -- -D warnings` is **red on files this slice did not touch**:

- `tests/doctor_early_dispatch.rs` (`Path::canonicalize` disallowed)
- `src/diagnostics/fix_tests.rs` (same)
- `benches/edit_highlight.rs` (`needless_range_loop`)
- `tests/settings_e2e.rs` (`unnecessary_min_or_max`)

Those are not Welcome / title brand. This slice did not mop them.

## Product change

Single display constant: `xai_grok_pager::client_identity::PRODUCT_CLI_NAME = "grok-oss"`. Config item name stays `grok`.

| Surface | Now |
|---------|-----|
| Welcome full + hero-inline badge | `Grok OSS` |
| Hero subtitle | `Thanks for trying Grok OSS, give feedback with /feedback!` |
| Tutorial list title + intro + `/tutorial` description | `Welcome to Grok OSS` / Grok OSS tips |
| Pager-minimal welcome card | `Grok OSS` |
| `TitleItem::Grok`, empty-title fallback, `TitleManager::reset` | `grok-oss` |
| `terminal_title_string` | `session - grok-oss` or bare `grok-oss` |

## Files

- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/client_identity.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/notifications/title.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/mod.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/welcome/mod.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/welcome/hero_box.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/tutorial.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/slash/commands/tutorial.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager-minimal/src/welcome.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/tests/pty_e2e/minimal/minimal_new_session_keeps_history_and_resets.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/tests/pty_e2e/minimal/minimal_slash_switches_from_fullscreen.rs`

## Leftover (honest)

- Workspace trust line still says **Grok Build may run or modify contents in this directory**. Same string in pager-minimal `auth.rs`. Not one of the four named Welcome surfaces.
- Auth copy still says **Grok Build is not yet available for this account.**
- Clap `about = "Grok Build TUI"` in `app/cli.rs` is still upstream.
- Feedback prompt **How can we improve Grok Build?** is still upstream.
- Billing chrome still says **Grok Build class**. That is the xAI invoice class name, not product Welcome brand.
- Resume-hint / pager-bin `PRODUCT_CLI_NAME` wire-up from Surmount `origin/main` was not restored. Prefer welcome + title only.
- User-guide still has no `grok-oss` / Grok OSS pages (not in this slice).
- PTY banner sentinels were updated to `Grok OSS`. Those PTY tests were not run.
- Window-title `TerminalTitleAction` / `terminal_title_osc_payload` helpers from `origin/main` were not restored. Named tests call `terminal_title_string` directly.
- `fmt` and lib clippy done. Package `--all-targets` clippy is still red on unrelated files listed above.
- Assertion-red was never captured in a cargo run. Green on the named filters is observed.
