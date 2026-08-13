# Cargo update vs GNOME keyring Unlock (2026-08-12)

**Verdict: FINE**

**This update is not why the keyring dialog appeared.** The dialogs came first. `cargo update` was after. The Linux Secret Service backend crate (`zbus-secret-service-keyring-store`) is still **1.0.0**. Unlock/Prompt is still the existing grok-oss `keyring::Entry` get/set path on a locked login collection.

## Operator summary

`keyring` 4.1.2 to 4.1.6 (compare [v4.1.2...v4.1.6](https://github.com/open-source-cooperative/keyring-rs/compare/v4.1.2...v4.1.6), accessed: 2026-08-12) is facade-only: first-call store-init error reporting, yanked 4.1.3 `Entry::new` DOA fix, clippy, concurrent store-create guard, Apple store pin. No new Unlock/Prompt, no new D-Bus methods, no new default backends. Default `v1` is still Apple + Windows + zbus Secret Service.

`apple-native-keyring-store` 1.0.1 is macOS/iOS only. `dbus` 0.9.12 and `zbus` 5.14.0 to 5.18.0 are transport/codec; they do not call Secret Service Unlock by themselves. `webbrowser` 1.0.6 to 1.2.2 opens only on `open` / `open_browser` (Unix `$BROWSER` parse + macOS objc2). No spawn on import. `reqwest` 0.12.28, `rustls-platform-verifier` 0.7.0, `landlock` 0.4.7, `process-wrap` 9.1.0, `command-fds` 0.3.3, `which` 8.0.5, `hostname` 0.4.2, `os_info` 3.15.0 are HTTP/TLS/sandbox/path probes. None of them pop GNOME Unlock.

Product still does `keyring::Entry::new` then `get_password` / `set_password` in `crates/codegen/xai-grok-shell/src/auth/credentials_store.rs`. That is the known fail-loud miss: a locked login collection still prompts.

## crates.io identity

| Crate | Owners | Repo |
|-------|--------|------|
| `keyring` 4.1.6 | `brotskydotcom` (publish), `hwchen` | https://github.com/open-source-cooperative/keyring-rs |
| `webbrowser` 1.2.2 | `amodm` | https://github.com/amodm/webbrowser-rs |
| `zbus` 5.18.0 | `zeenix` | https://github.com/z-galaxy/zbus |

Long-lived publishers. No owner swap in this bump. Checksums match crates.io.

## Watch (not this update)

Product must not call Secret Service Unlock/Prompt when the login collection is locked. That bug predates this lockfile. Keep watching `journalctl` / Secret Service callers for *other* processes. This Rust bump does not explain the dialog.
