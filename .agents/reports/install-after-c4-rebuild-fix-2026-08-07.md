# Install after C4 multipoll + /rebuild TUI fix (2026-08-07)

## Dogfood binary

| Item | Value |
|------|--------|
| Version | `grok-oss 0.2.111 (c87f66a61d94) [stable]` |
| Path | `~/.cargo/bin/grok-oss` |
| Git SHA | `c87f66a61d94fbc43cc1588709b5298149aad81f` (`c87f66a`) |
| Stripped | yes |

## Install notes

- Plain `just install` failed at link: host `fuse-ld=wild` + project `fuse-ld=mold` left undefined `drop_in_place` symbols under mold.
- Release rebuild used the same just install intent (no wild), with `RUSTFLAGS='-C force-unwind-tables=yes -C link-arg=-fuse-ld=lld'`, then strip + `install` to cargo bin (same steps as the justfile after the cargo build).
- Installs C4 multipoll flat-poll detector / `flatPollUnprovenDebit` export and `/rebuild` TUI restore-gate / quiet relaunch fixes for dogfood.

## Tests

| Filter | Command | Exit |
|--------|---------|------|
| Pager rebuild/relaunch | `cargo test -p xai-grok-pager --lib -- may_exec_relaunch rebuild_relaunch_has_no_post_restore plan_rebuild_relaunch_matches restore_blocked_hint exec_failure_hint` | **0** (7 passed) |
| Shell multipoll flat-poll | `cargo test -p xai-grok-shell --lib -- included_poll_history flat_poll` | **0** (15 passed) |

No git commit/add.
