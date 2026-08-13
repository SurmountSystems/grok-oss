# Cargo update review: crypto / TLS / AWS-LC / keyring

Date: 2026-08-12. Lock: `Cargo.lock` after operator `cargo update`. No `cargo update` re-run. Reverse edges reconstructed from lock + workspace/`crates/**/Cargo.toml` (no `cargo tree` in this pass). Lock checksums match crates.io for every high-risk version below.

## Verdict

| Family | Verdict | Why |
|--------|---------|-----|
| Crypto / hash | **WATCH** | First-seen **hmac 0.13.0** and **sha1 0.11.0** (plus digest 0.11 / sha2 0.11 / cipher 0.5). Authors and git tags match RustCrypto. Not yanked. New major lines, not a surprise publisher. |
| TLS / HTTP | **WATCH** | **rustls-platform-verifier 0.7.0** (0.x bump), **openssl-probe 0.2** via rustls-native-certs, **tokio 1.53.1**, dual **tokio-tungstenite 0.27 + 0.29**. rustls/hyper/reqwest/ureq/quinn are normal publishers. No yanked crate. |
| AWS / native crypto | **WATCH** | Huge but expected: **aws-sdk-s3 1.140.0** (caret `1`) and **aws-lc-sys 0.43.0**. 0.43 is a **normal AWS-LC line**, not a surprise crate. Native C/asm surface is large. |
| openssl-probe 0.1.6 → 0.2.1 | **WATCH** | **Repo/homepage swapped** alexcrichton → rustls. Documented handover (2025-12-29), owners still include alexcrichton + djc + rustls publishers. Tag `0.2.1` exists. Not yanked. |
| keyring 4.1.2 → 4.1.6 + apple-native-keyring-store 1.0.1 | **FINE** | Same author (`brotskydotcom`), same org repos, patch/tiny bump. |

No crate in this set looks malicious. No yanked versions in the locked set. No author swap except the documented openssl-probe rustls takeover.

---

## Direct vs transitive

Workspace pins (root `Cargo.toml`): `blake3 = "1"`, `keyring = "4"`, `sha1 = "0.10"`, `md5 = "0.8"`, `rand = "0.9"`, `reqwest = "0.12"` (rustls-tls), `rustls = "0.23"` + `aws-lc-rs`, `tokio = "1"`, `tokio-tungstenite = "0.27"`, `webpki-roots = "0.26"`.

Crate-level directs:

- `xai-grok-agent`: `zeroize = "1"` → **1.9.0**
- `xai-grok-shell`: `jsonwebtoken = "10"` → **10.4.0**; `keyring` / `keyring-core`
- `xai-file-utils`: `aws-sdk-s3`/`aws-config`/`aws-smithy-http-client` `= "1"` → entire AWS bump + **hmac 0.13** + **sha1 0.11**
- `xai-grok-mcp`: `reqwest = "0.13"` (rmcp) → **rustls-platform-verifier 0.7.0**
- `xai-grok-telemetry`: `sentry 0.42` → **ureq 3.3.0**; `mid = "4"` → **hmac-sha256 1.1.14**
- `xai-computer-hub-sdk` / `xai-grok-voice`: workspace tokio-tungstenite **0.27.0**
- `pdf_oxide` (workspace `0.3.43`, lock **0.3.77**): **aes 0.9.2**, cipher 0.5, getrandom 0.4
- `gcloud-storage` (workspace): jsonwebtoken + reqwest 0.13

## First-seen in this family

| Crate | Lock | Pulled by | Notes |
|-------|------|-----------|--------|
| **hmac 0.13.0** | new (0.12.1 stays) | `aws-sdk-s3` 1.140, `aws-sigv4` 1.5.1 | RustCrypto/MACs. Trusted-pub SHA `0236c8eb…` **equals** tag `hmac-v0.13.0`. Not yanked. |
| **sha1 0.11.0** | new (workspace still 0.10.7) | `aws-smithy-checksums` 0.65.0 | RustCrypto/hashes. Trusted-pub SHA `2f00175a…` **equals** tag `sha1-v0.11.0`. Not yanked. |
| **openssl-probe 0.2.1** | major | `rustls-native-certs` 0.8.4 | See handover below. |
| digest 0.11.3 / cipher 0.5.2 / sha2 0.11 / ctutils 0.4.2 / cmov 0.5.4 | new 0.11 line | hmac 0.13, sha1 0.11, aes 0.9 | RustCrypto digest 0.11 companions. |
| rand 0.10.2 / getrandom 0.4.3 / chacha20 0.10.1 | extra majors | `tokio-retry`, `quinn-proto`, `pdf_oxide` | rust-random trusted pub. 0.2.17 + 0.3.4 + 0.4.3 all present. |

## High-risk crate checks (crates.io + git tags)

None of these versions are yanked. Lock checksum == crates.io checksum for each.

- **aes 0.9.2**: RustCrypto/block-ciphers trusted pub. Also keep aes 0.8.4. Pulled by pdf_oxide.
- **blake3 1.8.5**: Jack O'Connor / BLAKE3-team. Direct workspace. Fine.
- **zeroize 1.9.0**: RustCrypto/utils trusted pub. Direct via agent.
- **jsonwebtoken 10.4.0**: Keats / github.com/Keats/jsonwebtoken. Fine.
- **hmac-sha256 1.1.14**: jedisct1 (Frank Denis). Fine. Not the RustCrypto `hmac` crate.
- **base64ct 1.8.3**, **constant_time_eq** 0.3.1 + 0.4.2: RustCrypto/utils. Fine.
- **rustls 0.23.43**: ctz, repo rustls/rustls, tag `v/0.23.43` exists. Checksum match.
- **rustls-pki-types 1.15.1**, **rustls-native-certs 0.8.4**, **webpki-roots** 0.26.11 + 1.0.9, **webpki-root-certs 1.0.9**: rustls org / djc.
- **rustls-platform-verifier 0.7.0**: djc, rustls org, tag `v/0.7.0` exists. 0.6.2 → 0.7.0 is a real 0.x bump (Linux/BSD cert load). Comes from **reqwest 0.13** (MCP + gcloud), not workspace 0.12.
- **hyper 1.11.0** / **reqwest 0.12.28**: seanmonstar. reqwest 0.13.4 also locked for MCP.
- **ureq 3.3.0**: algesten. Via sentry.
- **quinn 0.11.11**: via reqwest HTTP/3 path.
- **tokio 1.53.1**: Alice Ryhl / tokio-rs. Tag `tokio-1.53.1` exists.
- **tokio-tungstenite**: lock keeps **0.27.0** (workspace pin) **and** **0.29.0** (axum). 0.29 publisher daniel-abramov / snapview. Not yanked.
- **aws-lc-rs 1.17.3** / **aws-lc-sys 0.43.0**: published by `justsmth` (AWS). Repo `github.com/aws/aws-lc-rs`. Tags `v1.17.3` and `aws-lc-sys/v0.43.0` exist. crates.io latest sys is already **0.44.0**, so 0.43 is on-line, not a one-off. Sys crate is ~9.6 MiB native (C/asm). Versioning is documented as unstable; 0.39 → 0.43 in one update is the usual AWS-LC cadence, not a renamed crate.
- **aws-sdk-s3 1.140.0** (+ sso 1.105, ssooidc 1.107, sts 1.110, smithy-runtime 1.12.1, …): `aws-sdk-rust-ci`, repo awslabs/aws-sdk-rust. Caret `1` explains 1.109 → 1.140.
- **openssl-probe 0.2.1**: published by **djc**. Homepage/repo now `github.com/rustls/openssl-probe` (0.1.6 was alexcrichton). Owners: **alexcrichton + djc + github:rustls:publishers**. Tag `0.2.1` on the rustls fork. rustls 0.2.0 release notes call this an official handover (2025-12-29). rustls-native-certs 0.8.x exists to consume 0.2. **Watch the swap; do not treat as a hijack.**
- **keyring 4.1.6** / **apple-native-keyring-store 1.0.1**: Daniel Brotsky, open-source-cooperative. Fine.

## Operator summary

The update is a normal crates.io resolver walk, not a mystery crate drop.

hmac 0.13 and sha1 0.11 are first-seen because AWS SDK/smithy moved onto RustCrypto digest 0.11. Tags match trusted-publishing SHAs. Workspace `sha1 = "0.10"` is unchanged.

openssl-probe 0.2.1 really did change homepage to rustls. That is a published handover (alexcrichton still an owner). rustls-native-certs 0.8.4 is why it entered.

aws-lc-sys 0.43.0 is the stock AWS-LC sys crate for aws-lc-rs 1.17.3, not a surprise. Native blob is large; treat as build-time watch, not malware.

rustls-platform-verifier 0.7 is only on the reqwest 0.13 (MCP) path. Workspace reqwest 0.12 still uses rustls-native-certs.

tokio-tungstenite 0.29 is extra (axum). Product pin is still 0.27.

No yanked versions, no unexpected authors, no lock checksum mismatch, no evidence of a malicious crate.
