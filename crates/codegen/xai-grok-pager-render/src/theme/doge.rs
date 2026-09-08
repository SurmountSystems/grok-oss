//! DOGE theme + pure 8-colour palette / quantisation.
//!
//! Canonical theme id: **`doge`** (`ThemeKind::Doge`, display “DOGE”).
//! No parse aliases (`ecma-doge`, `rgbcmykw`, `ansi-8`, … are rejected).
//!
//! Palette rules: `doc/dev/specs/doge-pure-8-colour-2026-07-26.md`
//! (project internal note — not an ECMA standard). Pure primaries,
//! hard-threshold quantisation (channel ≥ 128 → 255), optional
//! Floyd–Steinberg helper for image buffers.
//!
//! Design intent: OLED-friendly true black canvas with only the classic
//! 3-bit primary set (Black Red Green Yellow Blue Magenta Cyan White),
//! matching ANSI / ECMA-48 / ISO 6429 SGR *names* and index order. No
//! mid-gray hex; muted UI roles use pure accents (cyan / yellow / white),
//! not `#808080` gray or ANSI DarkGray.
//!
//! **Grok OSS application semantic roles** (on top of the pure palette;
//! normative palette SoT: https://github.com/SurmountSystems/specs/blob/main/0001_DOGE.md
//! Clause 8 MAY define app roles):
//! - **Green** — Human (prompt pointer, left rail, OSC 12 cursor, success)
//! - **Magenta** — Agent (activity, model label, assistant/thinking chrome)
//! - **Yellow** — Dates, times, other useful context / secondary chrome
//! - **Cyan** — System tags, limits, credits, path/meta
//! - **Red / Blue** — Avoid unless contextually useful (errors stay red)
//! - **Gray / alpha** — Forbidden as theme palette colours
//!
//! Pure blue is not used for UI text slots (palette still includes blue
//! for quantisation). Do **not** claim measured power savings in product docs.

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

/// Hard-threshold: channel ≥ 128 → 255, else 0.
#[inline]
pub const fn hard_threshold_channel(channel: u8) -> u8 {
    if channel >= 128 { 255 } else { 0 }
}

/// Quantise one RGB triple to a DOGE pure primary (hard-threshold).
#[inline]
pub const fn quantise_rgb(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    (
        hard_threshold_channel(r),
        hard_threshold_channel(g),
        hard_threshold_channel(b),
    )
}

/// Quantise a [`Color::Rgb`] to DOGE pure primary RGB; other variants pass through.
pub fn quantise_color(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let (r, g, b) = quantise_rgb(r, g, b);
            Color::Rgb(r, g, b)
        }
        other => other,
    }
}

/// DOGE pure 8-colour palette in ANSI SGR index order (0…7).
pub const PALETTE: [(u8, u8, u8); 8] = [
    (0, 0, 0),       // 0 Black
    (255, 0, 0),     // 1 Red
    (0, 255, 0),     // 2 Green
    (255, 255, 0),   // 3 Yellow
    (0, 0, 255),     // 4 Blue
    (255, 0, 255),   // 5 Magenta
    (0, 255, 255),   // 6 Cyan
    (255, 255, 255), // 7 White
];

/// Hex strings for each DOGE pure colour (uppercase, with `#`).
pub const PALETTE_HEX: [&str; 8] = [
    "#000000", "#FF0000", "#00FF00", "#FFFF00", "#0000FF", "#FF00FF", "#00FFFF", "#FFFFFF",
];

/// ANSI / ECMA-48 SGR names for indices 0…7 (real standard name order).
pub const PALETTE_NAMES: [&str; 8] = [
    "Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White",
];

/// Index `0…7` from pure primary RGB via `R + 2·G + 4·B` (channels in `{0,255}`).
///
/// Non-pure inputs are hard-thresholded first.
#[inline]
pub const fn index_of_rgb(r: u8, g: u8, b: u8) -> u8 {
    let (r, g, b) = quantise_rgb(r, g, b);
    (r / 255) + 2 * (g / 255) + 4 * (b / 255)
}

/// Nearest DOGE pure colour by squared Euclidean distance in RGB space.
pub fn nearest_rgb(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let mut best = PALETTE[0];
    let mut best_d = u32::MAX;
    for &(pr, pg, pb) in &PALETTE {
        let dr = r as i32 - pr as i32;
        let dg = g as i32 - pg as i32;
        let db = b as i32 - pb as i32;
        let d = (dr * dr + dg * dg + db * db) as u32;
        if d < best_d {
            best_d = d;
            best = (pr, pg, pb);
        }
    }
    best
}

/// Format pure RGB as `#RRGGBB` (uppercase).
pub fn hex_of_rgb(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

// ── Optional: Floyd–Steinberg on a packed RGB buffer ─────────────────────

/// Apply Floyd–Steinberg error diffusion, quantising each pixel with hard-threshold.
///
/// `pixels` is a flat row-major buffer of `(R,G,B)` length `width * height`.
/// No-op when `width == 0` or the buffer is shorter than one row.
///
/// Optional helper for image buffers. Hard-threshold alone is sufficient
/// for theme / single-colour DOGE purity.
pub fn floyd_steinberg_quantise(pixels: &mut [(u8, u8, u8)], width: usize) {
    if width == 0 || pixels.is_empty() {
        return;
    }
    let height = pixels.len() / width;
    if height == 0 {
        return;
    }

    // Working buffer in i16 so error diffusion can undershoot/overshoot.
    let mut work: Vec<(i16, i16, i16)> = pixels
        .iter()
        .take(width * height)
        .map(|&(r, g, b)| (r as i16, g as i16, b as i16))
        .collect();

    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            let (or, og, ob) = work[i];
            let r = or.clamp(0, 255) as u8;
            let g = og.clamp(0, 255) as u8;
            let b = ob.clamp(0, 255) as u8;
            let (nr, ng, nb) = quantise_rgb(r, g, b);
            work[i] = (nr as i16, ng as i16, nb as i16);
            pixels[i] = (nr, ng, nb);

            let er = or - nr as i16;
            let eg = og - ng as i16;
            let eb = ob - nb as i16;

            // Classic FS weights: right 7/16, below-left 3/16, below 5/16, below-right 1/16.
            let distribute = |work: &mut [(i16, i16, i16)], idx: usize, num: i16| {
                let (wr, wg, wb) = work[idx];
                work[idx] = (wr + er * num / 16, wg + eg * num / 16, wb + eb * num / 16);
            };

            if x + 1 < width {
                distribute(&mut work, i + 1, 7);
            }
            if y + 1 < height {
                if x > 0 {
                    distribute(&mut work, i + width - 1, 3);
                }
                distribute(&mut work, i + width, 5);
                if x + 1 < width {
                    distribute(&mut work, i + width + 1, 1);
                }
            }
        }
    }
}

// ── Named palette colours as ratatui::Color ──────────────────────────────

/// Helper for concise const `Color::Rgb` definitions.
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// Classic 8 DOGE / ANSI primaries (pure RGBCMYKW set).
///
/// “ANSI” here means classic 3-bit SGR colour *names*, not a product theme id.
#[allow(dead_code)]
pub mod palette {
    use super::*;

    pub const BLACK: Color = rgb(0, 0, 0);
    pub const RED: Color = rgb(255, 0, 0);
    pub const GREEN: Color = rgb(0, 255, 0);
    pub const YELLOW: Color = rgb(255, 255, 0);
    pub const BLUE: Color = rgb(0, 0, 255);
    pub const MAGENTA: Color = rgb(255, 0, 255);
    pub const CYAN: Color = rgb(0, 255, 255);
    pub const WHITE: Color = rgb(255, 255, 255);
}

/// Map a terminal-palette green onto DOGE Human green `#00FF00`.
///
/// Named ANSI `Color::Green` / `Color::LightGreen` and the matching 16-color
/// / cube indexes follow the host palette (xterm normal green is `#00cd00`).
/// [0001_DOGE](https://github.com/SurmountSystems/specs/blob/main/0001_DOGE.md)
/// (accessed: 2026-08-31) requires `(0, 255, 0)`. Other colors pass through
/// so non-green Human accents (and `Reset` under `NO_COLOR`) stay unchanged.
pub fn as_doge_human_green(color: Color) -> Color {
    match color {
        Color::Green | Color::LightGreen => palette::GREEN,
        // 2 = ANSI normal green, 10 = bright green, 46 = 256-color cube (0,255,0).
        Color::Indexed(2 | 10 | 46) => palette::GREEN,
        other => other,
    }
}

impl Theme {
    /// DOGE — pure `#000` bg, `#fff` text/lines, pure 8-colour primaries only.
    ///
    /// Colors are defined in RGB. Call [`Theme::quantized`] to downgrade
    /// them to the terminal's supported color level before rendering.
    /// `requires_truecolor` is false: pure primaries quantize cleanly.
    ///
    /// **Context-bar solid-step contract** (keep in sync with
    /// `xai-grok-pager::views::context_bar` structural fingerprint fallback):
    /// `bg_base=black`, `text_primary=white`, `gray=yellow`, `warning=yellow`,
    /// `accent_error=red`, `accent_assistant=magenta`, `path=cyan`. Production
    /// gating is `ThemeKind::Doge` first; the fingerprint is a unit-test
    /// fallback for raw/quantized themes without a kind cache set.
    pub const fn doge() -> Self {
        use palette::*;
        Self {
            // All backgrounds pure black (no gray ramp).
            bg_base: BLACK,
            bg_light: BLACK,
            bg_dark: BLACK,
            bg_highlight: BLACK,
            bg_hover: BLACK,
            bg_terminal: BLACK,

            // Human chrome: prompt pointer, left rail, OSC 12 cursor → green.
            accent_user: GREEN,
            accent_assistant: MAGENTA,
            accent_thinking: MAGENTA,
            accent_tool: WHITE,
            // System tags / limits / credits family → cyan (free green for Human).
            accent_system: CYAN,
            accent_error: RED,
            // Success checkmarks stay green (same primary as Human).
            accent_success: GREEN,
            // Agent activity + subagent throbber / tool running accent → magenta.
            accent_running: MAGENTA,
            // Slash skills + skill tool accent → green (affordance family).
            accent_skill: GREEN,

            text_primary: WHITE,
            text_secondary: WHITE,

            // No mid-gray hex / silver: chromatic hierarchy for muted UI.
            // gray_dim = ambient meta (badges, starting-session spinner).
            // gray = secondary chrome (timers, activity meta, close chips).
            // gray_bright = near-body secondary.
            gray_dim: CYAN,
            gray: YELLOW,
            gray_bright: WHITE,

            command: YELLOW,
            path: CYAN,
            // Semantic running indicator chrome (activity) → magenta.
            running: MAGENTA,
            warning: YELLOW,

            fuzzy_accent: CYAN,

            accent_plan: YELLOW,
            accent_verify: MAGENTA,
            accent_feedback: CYAN,
            accent_remember: GREEN,

            // Bright borders/lines = white on black.
            selection_border: WHITE,
            prompt_border: WHITE,
            prompt_border_active: WHITE,
            hover_border: WHITE,

            // Prompt info-line model label (e.g. "Grok 4.5 (high)") → magenta.
            accent_model: MAGENTA,

            scrollbar_bg: BLACK,
            scrollbar_fg: WHITE,

            // Line-fg diff mode (bg black + solid red/green fg).
            diff_delete_bg: BLACK,
            diff_delete_fg: RED,
            diff_insert_bg: BLACK,
            diff_insert_fg: GREEN,
            diff_equal_fg: WHITE,
            diff_gutter_fg: WHITE,

            bg_visual: BLACK,

            paste_bg: BLACK,
            paste_fg: WHITE,
            paste_dim: WHITE,

            md_heading_h1: CYAN,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: MAGENTA,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: YELLOW,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: WHITE,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: WHITE,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: WHITE,
            md_heading_h6_mod: Modifier::empty(),
            md_code: CYAN,
            md_task_checked: GREEN,
            md_task_unchecked: WHITE,
            md_muted: WHITE,
            md_code_bg: BLACK,
            md_text: WHITE,
            // Pure green for links: higher luminance than blue/cyan on black
            // for most eyes; stays on the DOGE primary set.
            link_fg: GREEN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_doge_primary(c: Color) -> bool {
        matches!(
            c,
            Color::Rgb(0, 0, 0)
                | Color::Rgb(255, 0, 0)
                | Color::Rgb(0, 255, 0)
                | Color::Rgb(255, 255, 0)
                | Color::Rgb(0, 0, 255)
                | Color::Rgb(255, 0, 255)
                | Color::Rgb(0, 255, 255)
                | Color::Rgb(255, 255, 255)
        )
    }

    fn theme_colors(t: &Theme) -> [Color; 59] {
        [
            t.bg_base,
            t.bg_light,
            t.bg_dark,
            t.bg_highlight,
            t.bg_hover,
            t.bg_terminal,
            t.accent_user,
            t.accent_assistant,
            t.accent_thinking,
            t.accent_tool,
            t.accent_system,
            t.accent_error,
            t.accent_success,
            t.accent_running,
            t.accent_skill,
            t.text_primary,
            t.text_secondary,
            t.gray_dim,
            t.gray,
            t.gray_bright,
            t.command,
            t.path,
            t.running,
            t.warning,
            t.fuzzy_accent,
            t.accent_plan,
            t.accent_verify,
            t.accent_feedback,
            t.accent_remember,
            t.selection_border,
            t.hover_border,
            t.prompt_border,
            t.prompt_border_active,
            t.accent_model,
            t.scrollbar_bg,
            t.scrollbar_fg,
            t.diff_delete_bg,
            t.diff_delete_fg,
            t.diff_insert_bg,
            t.diff_insert_fg,
            t.diff_equal_fg,
            t.diff_gutter_fg,
            t.bg_visual,
            t.paste_bg,
            t.paste_fg,
            t.paste_dim,
            t.md_heading_h1,
            t.md_heading_h2,
            t.md_heading_h3,
            t.md_heading_h4,
            t.md_heading_h5,
            t.md_heading_h6,
            t.md_code,
            t.md_task_checked,
            t.md_task_unchecked,
            t.md_muted,
            t.md_code_bg,
            t.md_text,
            t.link_fg,
        ]
    }

    #[test]
    fn doge_uses_only_pure_primaries() {
        let t = Theme::doge();
        for c in theme_colors(&t) {
            assert!(is_doge_primary(c), "off-palette color: {c:?}");
        }
    }

    #[test]
    fn doge_pure_black_bg_white_body_and_primaries() {
        let t = Theme::doge();
        assert_eq!(t.bg_base, Color::Rgb(0, 0, 0));
        assert_eq!(t.bg_terminal, Color::Rgb(0, 0, 0));
        assert_eq!(t.md_code_bg, Color::Rgb(0, 0, 0));
        assert_eq!(t.text_primary, Color::Rgb(255, 255, 255));
        assert_eq!(t.selection_border, Color::Rgb(255, 255, 255));
        assert_eq!(t.accent_error, Color::Rgb(255, 0, 0));
        assert_eq!(t.accent_success, Color::Rgb(0, 255, 0));
        assert_eq!(t.command, Color::Rgb(255, 255, 0));
        assert_eq!(t.accent_user, Color::Rgb(0, 255, 0));
        assert_eq!(t.accent_system, Color::Rgb(0, 255, 255));
        assert_eq!(t.accent_assistant, Color::Rgb(255, 0, 255));
        assert_eq!(t.path, Color::Rgb(0, 255, 255));
        // No blue long body text.
        assert_ne!(t.text_primary, Color::Rgb(0, 0, 255));
        assert_ne!(t.md_text, Color::Rgb(0, 0, 255));
    }

    #[test]
    fn doge_link_fg_is_pure_green() {
        let t = Theme::doge();
        assert_eq!(
            t.link_fg,
            Color::Rgb(0, 255, 0),
            "DOGE markdown/hyperlink text must be pure green for visibility"
        );
        assert_ne!(t.link_fg, Color::Rgb(0, 0, 255), "must not use pure blue");
        assert_ne!(t.link_fg, Color::Rgb(0, 255, 255), "must not use cyan");
    }

    /// Agent activity animation + subagent throbber tokens are pure magenta.
    /// Skills/links stay green; path meta may stay cyan.
    #[test]
    fn doge_agent_activity_and_subagent_chrome_are_pure_magenta() {
        let t = Theme::doge();
        let magenta = Color::Rgb(255, 0, 255);
        let cyan = Color::Rgb(0, 255, 255);
        let green = Color::Rgb(0, 255, 0);

        // Paint paths: upper-right subagent indicator + lower-left / task
        // running accents read `theme.accent_running`; `theme.running` is the
        // semantic running indicator used for live activity chrome.
        assert_eq!(t.accent_running, magenta, "accent_running must be #FF00FF");
        assert_eq!(t.running, magenta, "running must be #FF00FF");
        assert_ne!(t.accent_running, cyan);
        assert_ne!(t.accent_running, green);
        assert_ne!(t.running, cyan);
        assert_ne!(t.running, green);

        // Non-agent chrome must not be dragged onto magenta by this contract.
        assert_eq!(t.accent_skill, green, "skills stay green");
        assert_eq!(t.link_fg, green, "links stay green");
        assert_eq!(t.path, cyan, "path meta may stay cyan");
    }

    /// Status/prompt chrome: model label token is magenta (not gray/cyan).
    #[test]
    fn doge_accent_model_is_pure_magenta_for_model_label() {
        let t = Theme::doge();
        assert_eq!(
            t.accent_model,
            Color::Rgb(255, 0, 255),
            "model label token must be DOGE magenta #FF00FF"
        );
        assert_eq!(
            t.text_primary,
            Color::Rgb(255, 255, 255),
            "branch chrome uses white"
        );
    }

    /// Product semantic roles on DOGE (application layer over pure 8-colour):
    /// Green = Human; Magenta = Agent; Yellow = dates/times/context;
    /// Cyan = system / limits / credits; no gray paint tokens.
    #[test]
    fn as_doge_human_green_named_ansi_is_rgb_0_255_0() {
        let spec = Color::Rgb(0, 255, 0);
        assert_ne!(
            Color::Green,
            spec,
            "named ANSI Green is not the DOGE RGB triple"
        );
        assert_eq!(as_doge_human_green(Color::Green), spec);
        assert_eq!(as_doge_human_green(Color::LightGreen), spec);
        assert_eq!(as_doge_human_green(Color::Indexed(2)), spec);
        assert_eq!(as_doge_human_green(Color::Indexed(10)), spec);
        assert_eq!(as_doge_human_green(Color::Indexed(46)), spec);
        assert_eq!(as_doge_human_green(spec), spec);
        assert_eq!(
            as_doge_human_green(Color::Magenta),
            Color::Magenta,
            "non-green Human-adjacent chrome must not snap to green"
        );
        assert_eq!(as_doge_human_green(Color::Reset), Color::Reset);
    }

    #[test]
    fn doge_accent_user_is_pure_green_for_human() {
        let t = Theme::doge();
        let green = Color::Rgb(0, 255, 0);
        assert_eq!(
            t.accent_user, green,
            "Human chrome (pointer, rail, OSC 12) must be pure green #00FF00"
        );
        assert_eq!(
            t.accent_success, green,
            "success stays green (same primary as Human)"
        );
        assert_ne!(
            t.accent_user,
            Color::Rgb(255, 255, 255),
            "Human is not white"
        );
        assert!(!matches!(t.accent_user, Color::Gray | Color::DarkGray));
    }

    #[test]
    fn doge_accent_system_is_pure_cyan_for_system_limits_credits() {
        let t = Theme::doge();
        let cyan = Color::Rgb(0, 255, 255);
        assert_eq!(
            t.accent_system, cyan,
            "system tags / limits chrome must be pure cyan #00FFFF (not green)"
        );
        assert_ne!(
            t.accent_system,
            Color::Rgb(0, 255, 0),
            "system must free green for Human"
        );
    }

    /// Green = Human + static affordances; magenta = agent activity / subagent
    /// chrome; cyan = system / path/meta (not agent motion); no pure-blue UI
    /// slots; muted hierarchy is chromatic (not mid-gray / silver).
    #[test]
    fn doge_roles_green_cyan_no_blue_ui_no_gray_text() {
        let t = Theme::doge();
        let green = Color::Rgb(0, 255, 0);
        let cyan = Color::Rgb(0, 255, 255);
        let magenta = Color::Rgb(255, 0, 255);
        let yellow = Color::Rgb(255, 255, 0);
        let white = Color::Rgb(255, 255, 255);
        let blue = Color::Rgb(0, 0, 255);

        // Human + static affordance → green
        assert_eq!(t.accent_user, green, "Human pointer / rail / cursor");
        assert_eq!(t.accent_skill, green, "slash skills");
        assert_eq!(t.link_fg, green, "links");
        assert_eq!(t.accent_success, green);
        assert_eq!(t.accent_remember, green);
        // System / limits / credits → cyan (not green)
        assert_eq!(t.accent_system, cyan, "system tags / limits chrome");

        // Agent activity + subagent chrome → pure magenta (not cyan/green)
        assert_eq!(
            t.accent_running, magenta,
            "subagent throbber / running accent"
        );
        assert_eq!(t.running, magenta, "running indicator chrome");
        assert_ne!(t.accent_running, cyan, "agent chrome must not be cyan");
        assert_ne!(t.accent_running, green, "agent chrome must not be green");
        assert_ne!(t.running, cyan, "running chrome must not be cyan");
        assert_ne!(t.running, green, "running chrome must not be green");

        // Path / search meta stay cyan; model label is magenta (agent chrome family)
        assert_eq!(t.path, cyan);
        assert_eq!(t.fuzzy_accent, cyan);
        assert_eq!(t.accent_feedback, cyan);
        assert_eq!(
            t.accent_model, magenta,
            "prompt info-line model label must be pure magenta, not gray/cyan"
        );

        // Muted hierarchy: no gray paint tokens
        assert_eq!(t.gray_dim, cyan, "ambient meta / dimmest chrome");
        assert_eq!(t.gray, yellow, "secondary chrome labels (not gray)");
        assert_eq!(t.gray_bright, white);
        assert_eq!(t.text_secondary, white);

        // No pure-blue semantic UI slots (blue stays in palette only).
        let ui_slots = [
            t.accent_system,
            t.accent_skill,
            t.accent_running,
            t.accent_user,
            t.accent_assistant,
            t.accent_thinking,
            t.accent_tool,
            t.accent_error,
            t.accent_success,
            t.text_primary,
            t.text_secondary,
            t.gray_dim,
            t.gray,
            t.gray_bright,
            t.command,
            t.path,
            t.running,
            t.warning,
            t.fuzzy_accent,
            t.accent_plan,
            t.accent_verify,
            t.accent_feedback,
            t.accent_remember,
            t.link_fg,
            t.md_text,
            t.md_muted,
        ];
        for c in ui_slots {
            assert_ne!(c, blue, "UI slot must not be pure blue: {c:?}");
            assert!(
                !matches!(c, Color::Gray | Color::DarkGray),
                "UI slot must not be ANSI gray: {c:?}"
            );
            if let Color::Rgb(r, g, b) = c {
                // Reject mid-gray RGB (equal channels not pure black/white).
                let is_mid_gray = r == g && g == b && r > 0 && r < 255;
                assert!(!is_mid_gray, "UI slot must not be mid-gray RGB: {c:?}");
            }
        }
    }

    /// Every canvas / sunken / elevated background slot is pure black —
    /// no charcoal wash, no gray ramp, no "light-bleed" elevation.
    #[test]
    fn doge_all_background_slots_are_pure_black() {
        let t = Theme::doge();
        let pure_black = Color::Rgb(0, 0, 0);
        let slots = [
            ("bg_base", t.bg_base),
            ("bg_light", t.bg_light),
            ("bg_dark", t.bg_dark),
            ("bg_highlight", t.bg_highlight),
            ("bg_hover", t.bg_hover),
            ("bg_terminal", t.bg_terminal),
            ("scrollbar_bg", t.scrollbar_bg),
            ("diff_delete_bg", t.diff_delete_bg),
            ("diff_insert_bg", t.diff_insert_bg),
            ("bg_visual", t.bg_visual),
            ("paste_bg", t.paste_bg),
            ("md_code_bg", t.md_code_bg),
        ];
        for (name, c) in slots {
            assert_eq!(c, pure_black, "{name} must be pure black #000000");
            if let Color::Rgb(r, g, b) = c {
                assert_eq!((r, g, b), (0, 0, 0));
            }
        }
    }

    #[test]
    fn quantise_pure_black_stays_pure_black() {
        assert_eq!(quantise_rgb(0, 0, 0), (0, 0, 0));
        assert_eq!(quantise_color(Color::Rgb(0, 0, 0)), Color::Rgb(0, 0, 0));
        // Sub-threshold near-blacks collapse to pure black (no charcoal).
        assert_eq!(quantise_rgb(1, 1, 1), (0, 0, 0));
        assert_eq!(quantise_rgb(127, 0, 0), (0, 0, 0));
        assert_eq!(quantise_rgb(127, 127, 127), (0, 0, 0));
        assert_eq!(quantise_color(Color::Rgb(40, 40, 40)), Color::Rgb(0, 0, 0));
    }

    #[test]
    fn quantize_color_pure_black_never_lifts_to_near_black() {
        use crate::render::color::indexed_to_rgb;
        use crate::theme::color_support::{ColorLevel, quantize_color};

        for level in [
            ColorLevel::TrueColor,
            ColorLevel::Ansi256,
            ColorLevel::Basic,
        ] {
            let q = quantize_color(Color::Rgb(0, 0, 0), level);
            match q {
                Color::Rgb(0, 0, 0) | Color::Black => {}
                Color::Indexed(n) => {
                    assert_eq!(
                        indexed_to_rgb(n),
                        (0, 0, 0),
                        "indexed {n} at {level:?} must resolve to pure black"
                    );
                }
                other => panic!("pure black must not quantize to {other:?} at {level:?}"),
            }
            // Never DarkGray / silver — those are the light-bleed slots.
            assert_ne!(q, Color::DarkGray, "at {level:?}");
            assert_ne!(q, Color::Gray, "at {level:?}");
        }
    }

    #[test]
    fn palette_black_constant_is_pure_rgb_zero() {
        assert_eq!(PALETTE[0], (0, 0, 0));
        assert_eq!(PALETTE_HEX[0], "#000000");
        assert_eq!(palette::BLACK, Color::Rgb(0, 0, 0));
    }

    #[test]
    fn doge_fixture_matches_palette_hex() {
        let t = Theme::doge();
        let pairs = [
            (t.bg_base, "#000000"),
            (t.text_primary, "#FFFFFF"),
            (t.accent_error, "#FF0000"),
            (t.accent_success, "#00FF00"),
            (t.command, "#FFFF00"),
            (t.accent_user, "#00FF00"),
            (t.accent_system, "#00FFFF"),
            (t.accent_assistant, "#FF00FF"),
            (t.path, "#00FFFF"),
            (t.accent_skill, "#00FF00"),
            (t.accent_running, "#FF00FF"),
            (t.running, "#FF00FF"),
            (t.link_fg, "#00FF00"),
            (t.gray_dim, "#00FFFF"),
            (t.gray, "#FFFF00"),
        ];
        for (color, hex) in pairs {
            let Color::Rgb(r, g, b) = color else {
                panic!("expected Rgb, got {color:?}");
            };
            assert_eq!(
                format!("#{r:02X}{g:02X}{b:02X}"),
                hex,
                "theme slot hex mismatch"
            );
            assert!(PALETTE_HEX.contains(&hex), "{hex} not in DOGE PALETTE_HEX");
            assert!(PALETTE.contains(&(r, g, b)));
        }
    }

    #[test]
    fn doge_slots_are_hard_threshold_fixed_points() {
        let t = Theme::doge();
        for c in theme_colors(&t) {
            let Color::Rgb(r, g, b) = c else {
                panic!("expected Rgb, got {c:?}");
            };
            assert_eq!(
                quantise_rgb(r, g, b),
                (r, g, b),
                "not a hard-threshold fixed point"
            );
        }
    }

    #[test]
    fn palette_hex_matches_rgb() {
        for (i, &(r, g, b)) in PALETTE.iter().enumerate() {
            assert_eq!(hex_of_rgb(r, g, b), PALETTE_HEX[i], "index {i}");
            assert_eq!(index_of_rgb(r, g, b), i as u8, "index formula {i}");
        }
    }

    #[test]
    fn palette_exact_channel_values() {
        assert_eq!(PALETTE[0], (0, 0, 0));
        assert_eq!(PALETTE[1], (255, 0, 0));
        assert_eq!(PALETTE[2], (0, 255, 0));
        assert_eq!(PALETTE[3], (255, 255, 0));
        assert_eq!(PALETTE[4], (0, 0, 255));
        assert_eq!(PALETTE[5], (255, 0, 255));
        assert_eq!(PALETTE[6], (0, 255, 255));
        assert_eq!(PALETTE[7], (255, 255, 255));
    }

    #[test]
    fn hard_threshold_channel_goldens() {
        assert_eq!(hard_threshold_channel(0), 0);
        assert_eq!(hard_threshold_channel(127), 0);
        assert_eq!(hard_threshold_channel(128), 255);
        assert_eq!(hard_threshold_channel(255), 255);
    }

    #[test]
    fn hard_threshold_rgb_goldens() {
        assert_eq!(quantise_rgb(0, 0, 0), (0, 0, 0));
        assert_eq!(quantise_rgb(127, 127, 127), (0, 0, 0));
        assert_eq!(quantise_rgb(128, 128, 128), (255, 255, 255));
        assert_eq!(quantise_rgb(255, 255, 255), (255, 255, 255));
        assert_eq!(quantise_rgb(200, 10, 10), (255, 0, 0));
        assert_eq!(quantise_rgb(10, 200, 10), (0, 255, 0));
        assert_eq!(quantise_rgb(10, 10, 200), (0, 0, 255));
        assert_eq!(quantise_rgb(200, 200, 10), (255, 255, 0));
        assert_eq!(quantise_rgb(200, 10, 200), (255, 0, 255));
        assert_eq!(quantise_rgb(10, 200, 200), (0, 255, 255));
    }

    #[test]
    fn quantise_preserves_named_colors() {
        assert_eq!(quantise_color(Color::Red), Color::Red);
        assert_eq!(
            quantise_color(Color::Rgb(200, 10, 10)),
            Color::Rgb(255, 0, 0)
        );
    }

    #[test]
    fn nearest_matches_hard_threshold_on_primaries() {
        for &(r, g, b) in &PALETTE {
            assert_eq!(nearest_rgb(r, g, b), (r, g, b));
        }
        // Mid gray → equal distance to black/white; implementation picks first
        // minimum (black). Hard threshold at 127 → black as well.
        assert_eq!(nearest_rgb(127, 127, 127), (0, 0, 0));
        assert_eq!(quantise_rgb(127, 127, 127), (0, 0, 0));
    }

    #[test]
    fn floyd_steinberg_solid_stays_primary() {
        let mut px = vec![(255, 0, 0); 4];
        floyd_steinberg_quantise(&mut px, 2);
        assert!(px.iter().all(|&c| c == (255, 0, 0)));
    }

    #[test]
    fn floyd_steinberg_empty_noop() {
        let mut px: Vec<(u8, u8, u8)> = vec![];
        floyd_steinberg_quantise(&mut px, 0);
        assert!(px.is_empty());
    }
}
