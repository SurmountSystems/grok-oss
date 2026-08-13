# Cargo update supply-chain review (2026-08-12)

**Verdict:** No surprise git remote rewrite. Locked git SHAs did **not** move. `cargo update` fetched the two already-pinned remotes and bumped crates.io versions. Host Cargo is rewriting crates.io through **menhera-cooldown** (10-day sparse proxy). Repo config does not.

Branch `onto-xai/b13fa526f511` HEAD `241f6f12260d0b977effb54f6f915b55b095d34e`. This agent had no shell, so `git show HEAD:Cargo.lock` was not run. Working lock git lines were compared to `Cargo.toml` pins, [xai-org/grok-build@b13fa526f511](https://github.com/xai-org/grok-build/blob/b13fa526f511/Cargo.lock) (accessed: 2026-08-12), and `origin/main` lock. All three agree on the same two git SHAs.

## Operator summary

1. **async-openai** stays `95b52ebdedf42143083cf3d6f0e0be7c84e9c808` (before and after). Remote is GitHub org `our-forks`, a public fork of `64bit/async-openai` created 2026-07-20. Pinned commit is `grok@xai.dev`, message "Add ReasoningEffort::Max (xAI fork of async-openai-v0.33.1)". Not a SurmountSystems remote. Repo remotes are only `origin` (`SurmountSystems/grok-oss`) and `xai-org` (`xai-org/grok-build`). Workspace `[patch.crates-io]` is the same URL+rev as upstream xAI. No URL rewrite.
2. **nucleo** stays `5b74652e482f7c07d827f18c6d21e7540c242c69` (short rev `5b74652`). Remote is the public Helix org (`helix-editor/nucleo`). Cargo still prints "Updating git repository" when it fetches a pinned rev.
3. Repo `.cargo/config.toml` has rustflags/env only. No `[source.*]`, no `replace-with`, no extra registry.
4. Host `~/.cargo/config.toml` (not in the repo) sets `[registry] default = "menhera-cooldown"`, `[source.crates-io] replace-with = "menhera-cooldown"`, and `[source.menhera-cooldown] registry = "sparse+https://index.crates.menhera.org/10d/"`. A second index `.../7d/` is named as the default registry. crates.io deps resolve through the 10-day index. Lock still records `registry+https://github.com/rust-lang/crates.io-index`.
5. **menhera-cooldown:** zero mentions in the repo or `Cargo.lock`. It is a public Cargo sparse-index proxy that withholds newly published crate versions for N days. See [crates.io Cooldown Proxy](https://www.menhera.org/crates-io-cooldown-proxy-mitigating-supply-chain-attacks/) (accessed: 2026-08-12). Downloads still come from `static.crates.io`.
6. `cargo update` does not edit `Cargo.toml`. Working pins match xAI. Only `Cargo.lock` should have changed. Working lock is **15548** lines. xAI `b13fa526f511` lock is about **15131** lines (that gap also includes Surmount-only members, not just this update). No `git diff --stat` vs local HEAD.
7. Git sources in the lock: only those two remotes (async-openai + macros, nucleo + nucleo-matcher). No other git, no `path` source lines, no vendor, no `denver.space` / menhera URL in the lock. Local path crates (`mermaid-to-svg`, `dagre_rust`, `graphlib_rust`, `ordered_hashmap`, workspace members) have no `source` field. Expected.
8. Sampled crates.io versions that moved vs xAI lock (`syn 2.0.119`, `reqwest 0.12.28`, `thiserror 2.0.19`, `rand 0.9.5`) are **not yanked** on crates.io. Full lock not scanned for yanked. No `cargo metadata` run.

## Git deps

| Crate | Owner | BEFORE (xAI lock + `Cargo.toml` pin) | AFTER (working lock) | Moved? |
|---|---|---|---|---|
| async-openai 0.33.1 + macros 0.1.1 | GitHub org `our-forks` (xAI fork of `64bit/async-openai`, not Surmount) | `95b52ebdedf42143083cf3d6f0e0be7c84e9c808` | same | no |
| nucleo 0.5.0 + nucleo-matcher 0.3.1 | `helix-editor` (Helix) | `5b74652e482f7c07d827f18c6d21e7540c242c69` | same | no |

`~/.cargo/git/checkouts/` has one checkout each: `async-openai-…/95b52eb/`, `nucleo-…/5b74652/`.

## Unusual registries / patch

- **Repo** `Cargo.toml` `[patch.crates-io]`: only `async-openai` -> `our-forks` at the SHA above. Same as xAI.
- **Repo** `.cargo/config.toml`: no source replace.
- **Host** (affects this `cargo update`): menhera-cooldown replace-with, 10d vs 7d index split as above.

## Yanked

No exhaustive pass. Four versions that did move vs the xAI lock are published and `yanked: false` on crates.io (accessed: 2026-08-12). Menhera filters freshness, not yank.

Did not run `cargo update`. Did not commit. Did not read `~/.cargo/credentials.toml`.
