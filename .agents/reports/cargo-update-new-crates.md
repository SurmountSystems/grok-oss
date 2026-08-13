# Cargo update supply-chain review (2026-08-12)

Branch: `onto-xai/b13fa526f511`. Scope: named additions and jumps only.
Lockfile source on every named crate: `registry+https://github.com/rust-lang/crates.io-index`.
Checksums checked against crates.io API match lockfile. No typosquats found.
No secrets read (`credentials.toml` not opened). Reverse deps from `Cargo.lock` (no `cargo tree`).

## Overall: WATCH

No WORRY item. Largest new surface is the already-chosen `pdf_oxide` jump (`0.3.46` → `0.3.77`, crate first-published 2025-11-05, same owner) which now pulls `office_oxide` + `zip 8` + `harfrust`. Host cooldown proxy is a known public crates.io delay, not a surprise registry.

## Operator summary

1. `Updating menhera-cooldown index` is **host** Cargo, not this repo. Repo `.cargo/config.toml` has rustflags only. Host `~/.cargo/config.toml` sets `default = "menhera-cooldown"` and `source.crates-io.replace-with = "menhera-cooldown"`.
2. Registry URL: `sparse+https://index.crates.menhera.org/7d/` (named registry) and `sparse+https://index.crates.menhera.org/10d/` (replace-with source). Public crates.io cooldown proxy. Index `config.json` still points `dl`/`api` at `static.crates.io` / `crates.io`. **FINE** as identity. **WATCH** host 7-day vs 10-day window mismatch (config slop, not a second vendor).
3. `office_oxide` / `pdf_oxide` are **not** Surmount crates. Workspace already depends on `pdf_oxide = 0.3.43` (`xai-grok-tools`). Same crates.io owner `yfedoseev` (Yury F.), repos `github.com/yfedoseev/{pdf,office}_oxide`, site oxide.fyi. `office_oxide` 0.1.8 first crate date 2026-04-28; this version 2026-07-22 (~21d, past 10d cooldown). ~369k downloads. **WATCH** (new sibling parser, ~29k LOC).
4. `pdf_oxide` 0.3.77 published 2026-07-28 (~15d old). ~557k downloads, 82 versions, ~6.6 MB / ~337k Rust lines. Frequent publishes. Checksum `cd381aa9…` matches crates.io. **WATCH** for size and cadence, not identity.
5. `zip 8.6.0` comes only from `office_oxide`. Real `zip` crate (`zip-rs/zip2`). `zip 3.0.0` stays. **FINE**.
6. `jni 0.22.4` + `jni-macros` + `jni-sys 0.4.1` are Android optional deps of `webbrowser 1.2.2` and `rustls-platform-verifier 0.7.0`. `jni 0.21.1` was already locked (cpal/oboe). Linux TUI does not need JNI at runtime. **FINE**.
7. `defmt` 1.1.1 + macros/parser: knurling-rs, first 2020-08-14, tens of millions of downloads. Pulled by `jiff` 0.2.35 / `jiff-core` (datetime; used via `env_logger` / `gix-date`). Embedded logger as a `no_std` feature, not a TUI logger. **FINE**.
8. `harfrust` 0.12.0: HarfBuzz Rust port (`behdad` / harfbuzz/harfrust), via `pdf_oxide` `system-fonts`. **FINE**.
9. `symlink` 0.1.0: chris-morgan, first 2017-01-27, via `tracing-appender` 0.2.5. **FINE**.
10. Clap/logger majors (`env_filter` 2.0.0, `anstream` 1.0.0, `clap_lex` 1.1.0, `syn` 3.0.3 added beside 2.0.119) are epage/dtolnay via `clap` 4.6.5 / `env_logger` 0.11.11. **FINE**.
11. `taffy` 0.12.2 via `pdf_oxide`. `pulldown-cmark-to-cmark` 22 via `prost-build`. **FINE**.
12. `obfstr` 0.4.6 **is** the real CasualX crate (`github.com/CasualX/obfstr`, first 2019-03-20, ~2.8M downloads). Checksum `7cf5f1ac…` matches crates.io. Published 2026-07-17 by CasualX. Workspace pin `obfstr = "0.4"` used by `xai-grok-pager`, `xai-grok-pager-bin`, `xai-grok-shell`, `xai-grok-telemetry` (optional). Identity **FINE**. Capability **WATCH** (string obfuscation in first-party code).
13. `webbrowser` 1.2.2: workspace dep; used by `xai-grok-mcp`, `xai-grok-shell`. **FINE**.
14. `landlock` 0.4.7 via `nono` 0.53.0. `dbus` 0.9.12 / `zbus` 5.18 patch/minor. `insta` 1.48.0 via `xai-grok-pager` tests (mitsuhiko). **FINE**.

## Per item

| Item | Verdict | Who / identity |
|------|---------|----------------|
| menhera-cooldown index | FINE (+ WATCH 7d/10d) | Public cooldown proxy, not project-owned, not unknown. See [Menhera crates.io cooldown](https://www.menhera.org/crates-io-cooldown-proxy-mitigating-supply-chain-attacks/) (accessed: 2026-08-12). |
| office_oxide 0.1.8 ADDED | WATCH | Only `pdf_oxide`. Owner yfedoseev. Not first-party. |
| pdf_oxide 0.3.46→0.3.77 | WATCH | Workspace `xai-grok-tools`. Same owner. Big crate, many patches, now depends on office_oxide + harfrust + taffy. |
| zip 8.6.0 ADDED | FINE | `office_oxide` → zip-rs/zip2. |
| jni 0.22.4, jni-macros, jni-sys 0.4.1 | FINE | Android of webbrowser + rustls-platform-verifier. |
| defmt 1.1.1 + macros + parser | FINE | `jiff` / `jiff-core`. knurling-rs. |
| harfrust 0.12.0 | FINE | `pdf_oxide` fonts. behdad. |
| symlink 0.1.0 | FINE | `tracing-appender`. chris-morgan 2017. |
| env_filter 0.1.4→2.0.0 | FINE | `env_logger`. epage. 2.0.0 published 2026-06-25. |
| anstream 0.6.21→1.0.0 | FINE | `clap_builder` / `env_logger`. epage. |
| clap_lex 0.7.6→1.1.0 | FINE | `clap_builder`. |
| syn 3.0.3 ADDED | FINE | dtolnay. Used by clap_derive and other macros. syn 1 + syn 2 remain. |
| taffy 0.10.1→0.12.2 | FINE | `pdf_oxide` layout. |
| pulldown-cmark-to-cmark 21→22 | FINE | `prost-build`. |
| obfstr 0.4.4→0.4.6 | FINE id / WATCH use | Real CasualX crate. First-party pager/shell/telemetry. |
| webbrowser 1.0.6→1.2.2 | FINE | mcp + shell. Brings jni on Android. |
| landlock 0.4.4→0.4.7 | FINE | `nono`. |
| dbus 0.9.11→0.9.12, zbus 5.14→5.18 | FINE | Known Linux IPC crates. |
| insta 1.43.2→1.48.0 | FINE | `xai-grok-pager` snapshots. mitsuhiko. 1.48.0 published 2026-06-11. |

## Method notes

- Did not run `cargo tree` (read-only, no shell). Reverse edges reconstructed from `Cargo.lock` dependency lists.
- Did not treat lockfile `source = crates.io-index` as proof of live download path. Host replace-with still fetches crate **bytes** from crates.io; the cooldown index only delays which versions are visible.
- `office_oxide` 0.1.8 and `pdf_oxide` 0.3.77 are both older than the 10-day cooldown window as of 2026-08-12.
