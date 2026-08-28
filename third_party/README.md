# Third-party crates that belong in grok-oss

This directory holds first-party-adjacent graph and mermaid ports
(dagre, graphlib, mermaid-to-svg, ordered_hashmap). It is not a dump of
crates.io copies for cargo-audit.

Audit-wave path copies of async-openai, syntect, bm25, rhai, pdf_oxide,
and ttf-parser were removed (2026-08-27) so `just install` is not
compiling those trees. `chacha20` stays a git tag pin in the root
`Cargo.toml`, not a folder here.
