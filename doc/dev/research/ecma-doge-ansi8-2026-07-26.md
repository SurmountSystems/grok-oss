# DOGE pure 8-colour research join (historical)

Date: 2026-07-26 · status: **landed** (then rename: identity is **DOGE** only;
no ECMA product brand; no `ansi-8` / `ecma-doge` aliases)

## Intent (original → corrected)

Originally framed as “ECMA-DOGE” (humorous formal-style writeup) and theme
`ansi-8`. **Corrected product identity:** theme id **`doge`**, display
**“DOGE”**, pure 8-colour palette only. The formal-style note is a **project
internal** palette note — **not** a real ECMA standard and **not** product
branding.

## Status

| Deliverable | Status |
|-------------|--------|
| Project note `doc/dev/specs/doge-pure-8-colour-2026-07-26.md` | Shipped (was `ecma-doge-1st-edition-…`; retitled) |
| Theme `doge` pure palette hex | Shipped (`theme::doge`) |
| `theme::doge` hard-threshold + nearest + FS helper + unit tests | Shipped |
| User-guide `06-theming` DOGE section (`doge` only) | Shipped |
| FORK / RESIDUAL bullets | Shipped |
| Wave 2 polish (solid steps, doge.tmTheme, hide_header extend) | Shipped |
| Remove `ecma-doge` / `rgbcmykw` / `ansi-8` parse aliases | Shipped |

## Product map

| Surface | Path |
|---------|------|
| Project note | `doc/dev/specs/doge-pure-8-colour-2026-07-26.md` |
| Quantise util | `crates/codegen/xai-grok-pager-render/src/theme/doge.rs` |
| Theme | `crates/codegen/xai-grok-pager-render/src/theme/doge.rs` |
| Kind | `theme/mod.rs` (`doge` only) |
| Settings labels | `xai-grok-pager/src/settings/defs.rs` |
| User guide | `xai-grok-pager/docs/user-guide/06-theming.md` |

## Validate

```bash
cargo test -p xai-grok-pager-render --lib -- theme doge
```

## Still open (not this slice)

- *(closed Wave 2 polish)* Context-bar solid DOGE steps — shipped
- *(closed Wave 2 polish)* Pure-primary `doge.tmTheme` for DOGE syntax — shipped
- *(closed Wave 2 polish)* `hide_header` zeros welcome + dashboard headers — shipped

## Prior notes

- Wishlist / H1+H2: `doc/dev/research/ui-hide-header-ansi8-2026-07-26.md` (historical name)
- Palette planning: `/tmp/grok-1000/grok-explore-ansi-palette.md` (session)
