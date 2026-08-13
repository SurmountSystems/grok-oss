# CI 197: theme cache auto-dark + hook `/dev/tty` probe

Date: 2026-08-12
Crates: `xai-grok-pager-render`, `xai-grok-hooks`
Scope: the two remaining `just ci` unit fails named in the implement prompt.

## Root cause

### Theme (`env_theme_auto_dark_*` and `env_theme_system_alias_*`)

Product auto-dark already maps to **DOGE** when `auto_dark_theme` is unset. That is the named fork contract (`to_theme_kind` Dark default, `resolve_auto_dark_system_returns_doge`, user-guide `06-theming`, catalog in `doc/dev/upstream-regression-filters.md`).

The two failing tests still asserted **GrokNight**. They came in with the 2026-08-05 monorepo sync and were never updated when the dark default became DOGE. Same resolve path as the passing sibling `resolve_from_config_auto_sets_auto_mode_dark`.

Observed red (old binary, `--nocapture`):

```
assertion `left == right` failed
  left: Doge
 right: GrokNight
```

Changing the product default back to GrokNight would have reddened the DOGE tests in the same `theme::cache::tests` verify filter. Tests were updated to the named DOGE contract. Watcher-arm (`is_auto_mode`) and the `system` alias path were already correct.

### Hooks (`test_hook_child_cannot_open_dev_tty`)

Product already calls `detach_command` (`setsid`, EPERM fallback `setpgid` + `TIOCNOTTY`). Detach works.

The probe was `exec 3>/dev/tty 2>/dev/null && exit 1 || exit 0`. The runner invokes `sh -c`. On this host `sh` is bash 5.3 in POSIX mode. A failed `exec` redirect exits the whole non-interactive shell, so `|| exit 0` never runs. Detached or not, the hook exits 1 when `/dev/tty` cannot be opened.

Observed red (old binary under `script` PTY; this agent has no ctty so the test otherwise skips):

```
hook child should not be able to open /dev/tty after setsid(), got Failed("exit code 1")
```

Host probe (no new scripts): `sh -c 'exec 3>/dev/tty …'` exits 1; `sh -c '(: >/dev/tty) …'` prints DETACHED and exits 0. Same pattern already used in `xai-grok-shell` local/streaming terminal tests.

## Files

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager-render/src/theme/cache.rs` | Auto-dark env tests expect `ThemeKind::Doge`; comments cite the DOGE contract. |
| `crates/codegen/xai-grok-hooks/src/runner/command.rs` | Probe is `(: >/dev/tty) 2>/dev/null && exit 1 \|\| exit 0`. Comment explains POSIX `exec` abort. |

No product resolution or detach logic changed.

## Red then green

| Test | Red | Green |
|------|-----|-------|
| `theme::cache::tests::env_theme_auto_dark_arms_watcher_and_resolves` | old bin: `left: Doge right: GrokNight` | `theme::cache::tests` 34 passed |
| `theme::cache::tests::env_theme_system_alias_arms_auto_on_both_resolve_paths` | old bin: same Doge vs GrokNight on the OSC 11 path | included in the 34 |
| `runner::command::tests::test_hook_child_cannot_open_dev_tty` | old bin + `script` PTY: `Failed("exit code 1")` | new bin + `script` PTY: ok (does not skip) |

Sibling still green (not rewritten): `resolve_auto_dark_system_returns_doge`.

## Verify

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-pager-render -p xai-grok-hooks` | 0 |
| `cargo test -p xai-grok-pager-render --lib theme::cache::tests` | **0** (34 passed) |
| `cargo test -p xai-grok-hooks --lib runner::command::tests::test_hook_child_cannot_open_dev_tty` | **0** (skips without ctty; PTY re-run ok) |
| `cargo clippy -p xai-grok-pager-render -p xai-grok-hooks --all-targets -- -D warnings` | **0** |

No git add/commit.
