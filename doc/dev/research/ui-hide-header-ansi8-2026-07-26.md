# Wishlist — hide TUI header + pure 8-colour theme

Date: 2026-07-26 · status: **shipped** (H1+H2; polish residual optional)

> **Shipped name correction:** product theme id is **`doge`** (display
> “DOGE”). Early wishlist used `ansi-8` / “ANSI 8 (OLED)” — those ids were
> never kept as aliases. No ECMA product branding.

## Intent

1. `[ui] hide_header` — hide the top agent status bar.
2. Theme **`doge`**: pure `#000` bg, `#fff` text/lines, classic 8 pure ANSI
   primaries only (Black Red Green Yellow Blue Magenta Cyan White).

Design motivation: OLED-friendly true black + primary colors. **No** measured
power claims in product docs.

## Naming (as shipped)

- Ship id: `doge` only (no `ansi-8` / `ansi` / `tty` / `oled` / `ecma-doge` aliases).
- Display: “DOGE”.
- Not “octal”; not CGA index order (BGR vs ANSI R+2G+4B).

## Plan

[`.agents/plans/plan-ui-hide-header-ansi8.md`](../../../.agents/plans/plan-ui-hide-header-ansi8.md)

## Explore

- Theme/header map (session): `/tmp/grok-1000/grok-explore-theme-header.md`
- Palette standards: `/tmp/grok-1000/grok-explore-ansi-palette.md`
