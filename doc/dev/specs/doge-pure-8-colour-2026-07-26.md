# DOGE pure 8-colour palette (project note)

**Project internal note — not an ECMA standard, not a formal standards body
document.** Structured for clarity only. Date: 2026-07-26.

Status: **product truth** for Grok OSS theme `doge` and pure 3-bit RGB
quantisation helpers.

Mnemonic order of the eight colours: **R G B C M Y K W** is *not* the
index order. Index order is the classic ANSI / ECMA-48 SGR name order
(**Black Red Green Yellow Blue Magenta Cyan White**). Product theme id:
**`doge`** only (no parse aliases).

---

## 1 Scope

This note defines:

1. A fixed **8-colour pure digital primary palette** (§4).
2. Mapping of those colours onto **ECMA-48 / ISO 6429 SGR** named indices
   (§5) — ECMA-48 here is the *real* terminal control standard for SGR
   colour *names*; this note does not invent or claim an ECMA product brand.
3. **Quantisation** of arbitrary 8-bit-per-channel RGB into the palette
   (§6).

It does **not** redefine ECMA-48 control sequences, claim measured OLED
power savings, or prescribe terminal-emulator private palette tables.
It does **not** claim compliance with any ECMA colour standard.

### 1.1 Product purity

- Theme and helper code that ships as **DOGE** **shall** use exactly the
  §4 hex values for the eight primaries (no mid-tone substitutes as
  palette entries).

---

## 2 Normative references

| Document | Role |
|----------|------|
| [Surmount specs `0001_DOGE.md`](https://github.com/SurmountSystems/specs/blob/main/0001_DOGE.md) (v1.0.0) | **External SoT** for the pure 3-bit RGBCMYKW palette (no gray/alpha as palette colours). Product semantic roles are application-defined (Clause 8 MAY). |
| ECMA-48 / ISO/IEC 6429 | Select Graphic Rendition (SGR) colour *names* and index order (RGB not fixed by the standard) |
| This note | Project annex: SGR index order, quantisation helpers, and **Grok OSS** semantic role map |

**Index-order note:** external `0001_DOGE.md` may list colours in RGBCMYKW
table order. This note and product code use classic **ANSI SGR name order**
(Black=0 … White=7). Same eight hex values; do not mix index formulas blindly.

Historical CGA/VGA IRGB attribute numbering (BGR bit order, ~`#AA` levels,
brown yellow) is **informative only** and **shall not** be used as the
§4 palette.

---

## 3 Terms and definitions

| Term | Definition |
|------|------------|
| **DOGE colour** | One of the eight colours in §4 |
| **Pure primary** | Channel values restricted to `{0, 255}` only |
| **Hard-threshold quantisation** | §6.1 per-channel mapping |
| **Emissive black** | `#000000` — design intent: subpixels off on emissive displays; product copy **shall not** claim measured power savings |
| **SGR index** | Integer `n` in `30+n` / `40+n` (normal) and `90+n` / `100+n` (bright extension) for `n ∈ 0…7` |

---

## 4 Palette

The eight DOGE colours **shall** be exactly:

| Index | Name | Hex | RGB `(R, G, B)` | Bits `(R,G,B)` |
|------:|------|-----|-----------------|----------------|
| 0 | Black | `#000000` | `(0, 0, 0)` | 000 |
| 1 | Red | `#FF0000` | `(255, 0, 0)` | 100 |
| 2 | Green | `#00FF00` | `(0, 255, 0)` | 010 |
| 3 | Yellow | `#FFFF00` | `(255, 255, 0)` | 110 |
| 4 | Blue | `#0000FF` | `(0, 0, 255)` | 001 |
| 5 | Magenta | `#FF00FF` | `(255, 0, 255)` | 101 |
| 6 | Cyan | `#00FFFF` | `(0, 255, 255)` | 011 |
| 7 | White | `#FFFFFF` | `(255, 255, 255)` | 111 |

### 4.1 Index formula

For pure primaries only:

```
index = (R/255) + 2·(G/255) + 4·(B/255)
```

with each channel in `{0, 255}`. This matches ANSI / ECMA-48 SGR name order
(**not** CGA BGR hardware order).

### 4.2 Forbidden as palette primaries

Mid-tone or “soft ANSI” substitutes (for example `#800000`, `#C0C0C0`,
xterm default dark reds, or gray ramps) **shall not** appear as the eight
primary palette entries of a DOGE theme.

---

## 5 SGR map

### 5.1 Normal intensity

| Index | Name | Foreground SGR | Background SGR |
|------:|------|----------------|----------------|
| 0 | Black | 30 | 40 |
| 1 | Red | 31 | 41 |
| 2 | Green | 32 | 42 |
| 3 | Yellow | 33 | 43 |
| 4 | Blue | 34 | 44 |
| 5 | Magenta | 35 | 45 |
| 6 | Cyan | 36 | 46 |
| 7 | White | 37 | 47 |

### 5.2 Bright extension

Common 16-colour extensions use SGR `90–97` / `100–107` for bright
foreground / background of the same named colours.

**DOGE rule:** bright **shall** map to the **same pure §4 value** as
normal for indices 0–7. Implementations **shall not** invent a mid-gray
“bright black” as a DOGE primary. Dim roles **may** use SGR faint / bold /
reverse, or white with a dimming modifier, rather than non-primary gray hex.

### 5.3 Truecolor encoding

When emitting 24-bit SGR (`38;2;R;G;B` / `48;2;R;G;B`), DOGE emitters
**shall** use the exact §4 channel triples.

---

## 6 Quantisation

### 6.1 Hard-threshold quantisation

Each 8-bit channel is mapped independently:

```
out = 255  if channel ≥ 128
out = 0    otherwise
```

Apply to `R`, `G`, and `B` of an input colour to obtain a DOGE pure primary
(always one of the eight §4 colours).

**Goldens (informative examples):**

| Input RGB | Output RGB | DOGE name |
|-----------|------------|-----------|
| `(0,0,0)` | `(0,0,0)` | Black |
| `(127,127,127)` | `(0,0,0)` | Black |
| `(128,128,128)` | `(255,255,255)` | White |
| `(255,255,255)` | `(255,255,255)` | White |
| `(200,10,10)` | `(255,0,0)` | Red |
| `(10,200,10)` | `(0,255,0)` | Green |
| `(10,10,200)` | `(0,0,255)` | Blue |
| `(200,200,10)` | `(255,255,0)` | Yellow |
| `(200,10,200)` | `(255,0,255)` | Magenta |
| `(10,200,200)` | `(0,255,255)` | Cyan |

### 6.2 Floyd–Steinberg dither (optional)

An implementation **may** apply Floyd–Steinberg error diffusion before or
while quantising a 2-D image to DOGE colours. When present:

1. For each pixel in left-to-right, top-to-bottom scan order, quantise with
   §6.1 (or nearest §4 colour).
2. Distribute the quantisation error to neighbouring not-yet-processed
   pixels with the classic weights `7/16`, `3/16`, `5/16`, `1/16`.

Absence of §6.2 **does not** affect §4 / §6.1 product purity.

### 6.3 Nearest colour (informative)

Nearest-of-8 by squared Euclidean distance in RGB space yields the same
result as §6.1 for all pure-primary outputs and is a useful API for
callers that already hold `(R,G,B)` triples.

---

## 7 Product mapping (informative)

| Product surface | Mapping |
|-----------------|---------|
| Theme id `doge` only | §4 primaries for all semantic theme slots that are palette colours |
| Display name | “DOGE” — OLED is design motivation only |
| Black background | `#000000` (pixel-off design intent; no power claims) |
| Helper crate / module | Hard-threshold §6.1 unit-tested; §6.2 optional |
| External palette SoT | https://github.com/SurmountSystems/specs/blob/main/0001_DOGE.md |

### 7.1 Grok OSS semantic roles (application layer)

Built on top of pure §4 colours. Not part of the external palette law;
product chrome only (user-guide `06-theming` DOGE section).

| Colour | Role |
|--------|------|
| **Green** | **Human** — user prompt pointer, left accent rail (`┃`), OSC 12 cursor, success, slash skills, links (`accent_user`, `accent_success`, `accent_skill`, `link_fg`) |
| **Magenta** | **Agent** — running activity, throbbers, model label, assistant/thinking (`accent_running`, `running`, `accent_model`, `accent_assistant`, `accent_thinking`) |
| **Yellow** | Dates, times, timers, secondary context chrome (`gray` token paints yellow, `command`, `warning`, `accent_plan`) |
| **Cyan** | System tags, limits, credits, path/meta (`accent_system`, `path`, `gray_dim`, `accent_feedback`, `fuzzy_accent`) |
| **Red / Blue** | Avoid unless contextually useful (errors: `accent_error` red). Pure blue not used for UI text slots. |
| **Gray / alpha** | Forbidden as theme palette colours. Runtime dim/blend may still emit non-pure RGB during animation; treat remaining leaks as residual scrub work. |

**Human left rail:** every `UserPromptBlock` returns a static accent in
`accent_user` (green on DOGE). Paint path is shared with Recap
(`glyphs::accent_bar()`, `HorizontalLayout::ACCENT`). Idle expanded Recap
stays white (`accent_tool`).

### 7.2 Design notes (non-normative)

- Prefer cyan over pure blue for long body text or thin links on black
  (contrast). Links on DOGE use green for luminance.
- Reserve pure blue for sparse chrome / quantisation only.
- No mid-gray hex in the theme primary set; dim via modifiers if needed.

---

## 8 Document control

| Field | Value |
|-------|-------|
| Title | DOGE pure 8-colour palette (project note) |
| Date | 2026-07-26 |
| Product tree | Surmount Grok OSS |
| Spec path | `doc/dev/specs/doge-pure-8-colour-2026-07-26.md` |
| Research join | `doc/dev/research/ecma-doge-ansi8-2026-07-26.md` (historical; theme is `doge`) |

End of project note.
