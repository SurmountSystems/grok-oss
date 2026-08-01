# Plan: Hide TUI header + DOGE pure 8-colour theme

> **Shipped as:** theme id **`doge`**, display **“DOGE”**. Early draft names
> (`ansi-8`, “ANSI 8 (OLED)”, later `ecma-doge` aliases) are **not** parse
> aliases. DOGE is a product theme name only — not an ECMA standard or brand.
> Palette project note: `doc/dev/specs/doge-pure-8-colour-2026-07-26.md`.

## Context

**Wishlist (UDAX / UX):**

1. Config option under UI/theme surface to **hide the terminal UI header**
   (the top 1-row agent status bar: cwd/git left, context/queue/plan/todo right).
2. A **high-contrast dark theme** with pure **`#000000` background**, pure
   **`#ffffff` text/lines**, and remaining semantic colors from the classic
   **3-bit / 8-color ANSI** primary set only:

   | ANSI name | Hex |
   |-----------|-----|
   | Black | `#000000` |
   | Red | `#ff0000` |
   | Green | `#00ff00` |
   | Yellow | `#ffff00` |
   | Blue | `#0000ff` |
   | Magenta | `#ff00ff` |
   | Cyan | `#00ffff` |
   | White | `#ffffff` |

**Naming:** Prefer traditional **ANSI SGR names and order** (Black, Red, Green,
Yellow, Blue, Magenta, Cyan, White — ECMA-48 / ISO 6429 lineage). Do **not**
ship the id as “octal” (base-8 jargon) or as a faithful CGA hardware palette
(CGA used BGR index order and ~`#AA` levels / brown yellow). Map theme slots by
**ANSI name**, not CGA index tables.

**Motivation (design only):** pure black + primary drive patterns are friendlier
to emissive OLED-style displays (pixels off at true black). **Do not** claim
measured power savings in product copy or docs.

**Constraints:**

- Existing themes live in `xai-grok-pager-render` (`Theme` + `ThemeKind`).
- Config is flat **`[ui]`** (`UiConfig`) — not a new `[theme]` table.
- Minimal screen mode already forces `Theme::terminal_default()` and **ignores**
  theme settings; hide-header is primarily for **fullscreen agent** chrome.
- No product code until this plan is approved.

**Non-goals (this plan):**

- User-editable arbitrary hex palettes
- Measured OLED battery metrics
- Faithful CGA brown / 16-color PC palette
- Hiding bottom shortcuts bar, sticky scrollback block headers, or minimal-mode
  panel chrome (unless a later residual expands scope)
- Reworking every hardcoded HUD color (fps/scroll debug) in the first slice

**Assumptions:**

- “Header” means the **top agent status bar**, not welcome dashboard
  `top_bar` alone (welcome is optional stretch if low cost).
- Recommended theme id: **`ansi-8`** (aliases: `ansi`, `tty`, `oled`,
  `oled-ansi`). Display name: **“ANSI 8”** or **“ANSI 8 (OLED)”**.
- Config key for chrome: **`hide_header`** (bool, default `false`) under
  `[ui]` — readable and matches the user’s request; docs clarify it is the
  status bar.

## Approach

Two independent slices that share docs; either can ship alone.

### H1 — `[ui] hide_header`

- Add `hide_header: bool` (default `false`) to `UiConfig`.
- Wire settings registry (Appearance) + live setter (mirror `compact_mode`).
- In `AgentViewLayout::compute`, when set: status bar `Constraint::Length(0)`
  (same pattern as turn-status / banner height gates).
- Skip status paint and mouse hit-tests when height is 0.
- Document: loses cwd click, context %, queue badge, plan chip, goal line —
  intentional for max content area / OLED focus.
- Fullscreen agent first; dashboard welcome header only if trivial.

### H2 — Theme `ansi-8`

- New `theme/ansi_8.rs` (or `ansi8.rs`) implementing `Theme::ansi_8()` filling
  every `Theme` semantic field from the 8-color set only.
- `ThemeKind::Ansi8` + `from_name` / `display_name` / `ALL` / settings choices /
  slash `/theme` list.
- `requires_truecolor: false` (pure primaries quantize cleanly).
- **Semantic mapping (recommended):**

  | Role family | Color |
  |-------------|--------|
  | Backgrounds, code bg, scrollbar track | Black |
  | Primary text, bright borders/lines | White |
  | Dim / secondary text | White + `Modifier::DIM` (no mid-gray hex) |
  | Error / delete | Red |
  | Success / insert / remember | Green |
  | Warning / plan / command | Yellow |
  | Paths / running / links-ish | Cyan |
  | Assistant / thinking / verify accents | Magenta |
  | Sparse system/skill chrome | Blue (**not** long body text) |

- Syntect: reuse dark night `.tmTheme` for v1 (syntax may not be pure-primary;
  acceptable residual polish).
- Context-bar gradients: prefer solid steps from the 8 set (avoid lerp to
  off-palette grays when kind is Ansi8) if cheap; else document residual.

**Not:**

- **Not “octal” as shipping id** — jargon; ANSI names are the standard.
- **Not CGA-index wiring** — red/blue swap at index 1 vs 4.
- **Not half-scale `#80` dim primaries** for v1 unless DIM modifier is
  insufficient for chrome (prefer DIM first to stay pure).
- **Not a separate `[theme]` config table** — stay on `[ui].theme` +
  `[ui].hide_header`.

## Critical files

| Path | Why |
|------|-----|
| `crates/codegen/xai-grok-shared/src/ui_config.rs` | `hide_header` field + defaults |
| `crates/codegen/xai-grok-pager/src/settings/defs.rs` | Settings meta / THEME_CHOICES |
| `crates/codegen/xai-grok-pager/src/views/agent.rs` | `AgentViewLayout` status row height |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Status bar paint + hits |
| `crates/codegen/xai-grok-pager/src/views/agent_status.rs` | Status bar widget (maybe no-op path) |
| `crates/codegen/xai-grok-pager-render/src/theme/mod.rs` | `ThemeKind`, name maps |
| `crates/codegen/xai-grok-pager-render/src/theme/tokyonight.rs` | `Theme` struct field list |
| `crates/codegen/xai-grok-pager-render/src/theme/groknight.rs` | Copy pattern for new builder |
| `crates/codegen/xai-grok-pager-render/src/theme/ansi_8.rs` | **new** palette |
| `crates/codegen/xai-grok-pager/src/slash/commands/theme.rs` | `/theme` listing if hardcoded |
| `crates/codegen/xai-grok-pager/docs/user-guide/06-theming.md` | User-facing docs |
| `FORK.md` | Short product note when shipping |

## Reuse

| Symbol / module | Path | How |
|-----------------|------|-----|
| `UiConfig` + settings Appearance | shared + pager settings | Add bool like `compact_mode` |
| `Theme::groknight()` | `theme/groknight.rs` | Mirror structure; pure palette |
| `Theme::terminal_default()` | `theme/terminal_default.rs` | Contrast: named ANSI vs pure RGB hex |
| Height-gated layout chunks | `AgentViewLayout` | Mirror banner / turn_status 0-height |
| `ThemeKind::from_name` / settings choices | theme/mod + defs | Add Ansi8 + aliases |

## Steps

1. **H1 config + layout gate** — `hide_header` on `UiConfig`; layout
   `status_bar` height 0; skip paint/hits; unit tests on layout.
2. **H1 settings + docs** — settings toggle, optional live apply, short
   theming/appearance doc note.
3. **H2 palette + ThemeKind** — `Theme::ansi_8()` mapping table above; kind +
   aliases (`ansi-8`, `ansi`, `tty`, `oled`); settings + `/theme`.
4. **H2 tests + docs** — `from_name` goldens; 06-theming table row; FORK
   one-liner; note OLED motivation without power claims.
5. **Optional polish (same PR if small, else residual)** — context-bar
   solid steps for Ansi8; syntax theme; dashboard top bar hide when
   `hide_header`.

**Dependency:** 1→2 independent of 3→4; can parallelize H1 and H2 after step 0
file inventory.

## Risks

| Risk | Mitigation |
|------|------------|
| Blue `#0000ff` on black is hard to read | No blue body text; blue only sparse chrome; prefer cyan for paths/links |
| Yellow glare / warning fatigue | Yellow for rare warning/plan only |
| Many Theme fields, only 8 colors | Document mapping table; collapse grays to white+DIM |
| Hide header loses context % / queue / plan chip | Document; user opt-in only |
| Hit-tests / click targets on zero-height rect | Explicit skip when hidden |
| Minimal mode ignores themes | Document hide_header as fullscreen agent; no-op or N/A in minimal |
| Off-theme hardcoded HUD colors | Out of scope v1; residual list |
| Syntax highlighting not pure-primary | Accept reuse of night tmTheme for v1 |
| Bright-black-as-gray assumptions | Collapse bright black to black; no invented gray |

## Verification

```bash
# After implementation
cargo test -p xai-grok-shared --lib -- ui_config
cargo test -p xai-grok-pager-render --lib -- theme
# layout / settings tests as named by implementer
```

Manual:

1. `theme = "ansi-8"` in `~/.grok/config.toml` (or `/theme ansi-8`) → pure
   black bg, white text, primaries only on chrome accents.
2. `hide_header = true` → no top status row; transcript gains one line;
   no click crashes on old status rects.
3. Both together: max content + OLED palette.
4. `hide_header = false` + other themes unchanged (no regression).

## Open questions

(Non-blocking; recommended defaults in Approach — revise in chat if needed.)

1. Display label: **“ANSI 8”** vs **“ANSI 8 (OLED)”**?
   - *Recommend:* **“ANSI 8 (OLED)”** so the motivation is findable; id stays
     `ansi-8`.
2. Hide **welcome** dashboard location bar too when `hide_header`?
   - *Recommend:* fullscreen agent only in H1; welcome as polish if free.
3. Collapse bright black to pure black (strict) vs one mid-gray for disabled?
   - *Recommend:* strict pure set + DIM; no mid-gray hex in v1.

## Residual after this plan (not in steps)

- Context-bar / HUD hardcode audit for off-palette RGB
- Dedicated pure-primary syntax `.tmTheme`
- Optional hide shortcuts bar (separate key if ever wanted)
- TOON T3+ and other open RESIDUAL items unchanged

## Explore join notes

- `/tmp/grok-1000/grok-explore-theme-header.md`
- `/tmp/grok-1000/grok-explore-ansi-palette.md`
- Research pin (optional durable): `doc/dev/research/ui-hide-header-ansi8-2026-07-26.md`
  (copy of palette + intent if implementer wants tree-local brief)

## Effort

~1.5–3 eng-days combined (H1 ~0.5–1.5d, H2 ~0.5–1d + polish).
