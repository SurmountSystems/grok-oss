# `just install` verify: `grok-oss --version` ENXIO (os error 6)

**Date:** 2026-08-13
**Crates:** `xai-grok-pager`, `xai-grok-pager-bin`
**Contract:** `--version` prints version to stdout and exits 0 with no terminal device

---

## Recipe line

`justfile` `install` verify is line 386 (recipe starts at 373):

```
385:    @echo "==> verify"
386:    "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" --version
```

Same verify shape on `install-dist` (line 410) and `install-nix` (line 422). The copy and strip succeeded. Verify is what failed.

`/rebuild` runs that recipe through `run_command_captured` in `crates/codegen/xai-grok-update/src/rebuild.rs`: stdin `/dev/null`, stdout and stderr piped, `detach_std_command` (`setsid`, no controlling terminal). `TERM=dumb`. Progress is sanitized. Child stdio is never inherited. That part of the docs is true; verified in code.

## What `--version` opened

Clap already parsed `--version` / `-v` / `-V` as `PagerArgs.version` (`disable_version_flag = true`, custom flag). Tests called this "early version intent." **Nothing in `main` read the flag.**

`just install` uses the flag, not the `version` subcommand.

So `--version` looked like a bare interactive launch (`command.is_none()`). After crash handler, sentry, docs extract, sandbox, and tokio, it called `xai_grok_pager::app::run` → `init_terminal` → `crossterm::terminal::enable_raw_mode()`. That ioctls stdin and, when stdin is not a TTY, opens `/dev/tty`. With no controlling terminal, `open("/dev/tty")` returns **ENXIO** (os error 6). `main` printed `Error: {e:#}` and exited 1.

The `version` **subcommand** already printed and returned `Ok(())` inside `async_main`. Reproduced: `grok-oss version </dev/null` printed `grok 1.0.3 (…)` and exited 0 on the same installed binary that failed `--version`.

`--version` does **not** need Secret Service, a session socket, or rustix `isatty` on a live TUI. Those never ran if we had dispatched the flag.

## Root cause of ENXIO

Not a strip bug. Not a missing binary. Not the verify step being wrong.

`--version` was never dispatched, so install verify started the TUI under `/rebuild`'s captured stdio. Terminal init failed with ENXIO. The error text `Error: No such device or address (os error 6)` is the product `eprintln` in `main` after `async_main` returns `Err`.

## TDD

### Red (before product edit)

Installed and debug binaries, same fail:

```
Error: No such device or address (os error 6)
```

Command:

```
cargo test -p xai-grok-pager-bin --test version_without_tty -- --nocapture
```

All three failed (0.07s after compile):

- `version_flag_exits_zero_when_stdin_is_dev_null`
- `version_flag_exits_zero_when_stdin_pipe_is_closed`
- `version_flag_exits_zero_when_rebuild_captures_stdio`

Host check on the then-installed release binary (same text, exit 1):

```
grok-oss --version </dev/null
grok-oss --version <&-
setsid grok-oss --version </dev/null
```

### Product change

Smallest hermetic fix: after the mermaid-worker intercept, if `PagerArgs::parse_cli().version_only_json()` is `Some`, print version and return. No memtrace, requirements, sentry, docs, crash handler, sandbox, tokio, or TUI.

- `PagerArgs::version_only_json()` in `crates/codegen/xai-grok-pager/src/app/cli.rs`
- `print_cli_version` + early return in `crates/codegen/xai-grok-pager-bin/src/main.rs`
- `update --version <semver>` is **not** version-only (existing clap field)

Verify step left in place.

### Green

```
cargo fmt -p xai-grok-pager -p xai-grok-pager-bin
cargo clippy -p xai-grok-pager --lib --bins -- -D warnings
cargo clippy -p xai-grok-pager-bin --bins -- -D warnings
cargo test -p xai-grok-pager --lib -- version_flags_parse version_subcommand_is_version_only update_semver_flag_is_not_version_only ordinary_and_doctor_parsing
cargo test -p xai-grok-pager-bin --test version_without_tty -- --nocapture
```

Clippy: exit 0. Unit tests: 4 passed. Integration: 3 passed in 0.04s.

## Leftover honesty

- `~/.cargo/bin/grok-oss` is still the **pre-fix** release until the operator runs `just install` / `/rebuild` again. The debug test binary is the one that is green.
- Did not re-run full `just install` (release, ~17 minutes). The named contract is covered by the integration test, which is the same argv and stdio shape as verify.
- `grok doctor` still has `unreachable!("doctor was consumed before runtime startup")` in `async_main` with no matching early dispatch in this `main`. Pre-existing. Not this bug.
- `--version` no longer runs `validate_requirements`. Install verify will succeed even if a managed version policy would have exited 2. That is what a hermetic version check should do.
