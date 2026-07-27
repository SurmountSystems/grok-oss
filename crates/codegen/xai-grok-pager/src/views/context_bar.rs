//! Context usage bar — shows token usage in the status bar.
//!
//! Default builds a `Line<'static>` of styled spans: `8.5K / 1.0M` (actual tokens,
//! colored by usage percentage). On hover, replaces the tokens with a progress
//! bar + percentage, e.g. `█████ 42.0%`. The bar width is derived from the
//! default string length so the hover line is the same total width — no layout
//! shift on hover. The default is right-padded to a minimum of 6 columns so the
//! width invariant holds even for degenerate inputs like `0 / 9`.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use super::progress_bar::progress_bar_spans;
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Formatting utilities
// ---------------------------------------------------------------------------

/// Format a percentage as a fixed-width 5-char string.
///
/// - `< 10`:  `"X.XX%"` (e.g. `"0.00%"`, `"5.12%"`)
/// - `10–99`: `"XX.X%"` (e.g. `"20.1%"`, `"99.9%"`)
/// - `≥ 100`: `"MAX %"`
pub fn fmt_pct5(pct: f64) -> String {
    if pct >= 100.0 {
        "MAX %".to_string()
    } else if pct < 10.0 {
        format!("{pct:.2}%")
    } else {
        format!("{pct:.1}%")
    }
}

/// Format a token count as a compact string (≤4 chars).
///
/// - `0–999`:     `"0"`, `"12"`, `"999"`
/// - `1K–9.9K`:   `"1.2K"` (4 chars)
/// - `10K–999K`:  `"12K"`, `"999K"` (≤4 chars)
/// - `1M–9.9M`:   `"1.2M"` (4 chars)
/// - `10M+`:      `"12M"`, `"123M"` (≤4 chars)
pub fn fmt_tokens(n: u64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 10_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else if n < 1_000_000 {
        format!("{}K", n / 1_000)
    } else if n < 10_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else {
        format!("{}M", n / 1_000_000)
    }
}

// ---------------------------------------------------------------------------
// Color blending
// ---------------------------------------------------------------------------

/// A breakpoint for color blending: at `pct` percent, the bar color is `color`.
#[derive(Debug, Clone, Copy)]
pub struct ColorBreakpoint {
    pub pct: f64,
    pub color: Color,
}

/// Default breakpoints: text_primary → accent_user → warning → accent_error.
///
/// Breakpoint colors are raw RGB. The final color produced by [`blend_color`]
/// is quantized by the caller (see [`context_bar_line`]) so the output always
/// matches the terminal's capability level.
pub fn default_breakpoints(theme: &Theme) -> Vec<ColorBreakpoint> {
    vec![
        ColorBreakpoint {
            pct: 0.0,
            color: theme.text_primary,
        },
        ColorBreakpoint {
            pct: 50.0,
            color: theme.accent_user,
        },
        ColorBreakpoint {
            pct: 65.0,
            color: theme.accent_user,
        },
        ColorBreakpoint {
            pct: 75.0,
            color: theme.warning,
        },
        ColorBreakpoint {
            pct: 85.0,
            color: theme.warning,
        },
        ColorBreakpoint {
            pct: 95.0,
            color: theme.accent_error,
        },
    ]
}

/// Blend between breakpoints for a given percentage.
///
/// Default themes lerp between neighbouring breakpoints. DOGE themes use
/// solid steps only (no mid-segment lerp) so intermediate usage never
/// invents off-palette grays — see [`blend_color_with_mode`].
pub fn blend_color(pct: f64, breakpoints: &[ColorBreakpoint]) -> Color {
    blend_color_with_mode(pct, breakpoints, false)
}

/// Like [`blend_color`], but when `solid_steps` is true the colour jumps at
/// breakpoints instead of lerping (DOGE pure 8-colour palette stays pure).
///
/// Solid mode is a **right-closed step function**: at an exact breakpoint
/// percentage the colour of that breakpoint is used (e.g. 95% → error red).
/// Between breakpoints the lower step is held until the next threshold.
pub fn blend_color_with_mode(
    pct: f64,
    breakpoints: &[ColorBreakpoint],
    solid_steps: bool,
) -> Color {
    if breakpoints.is_empty() {
        return Color::Reset;
    }
    if solid_steps {
        // Last breakpoint with `pct >= bp.pct` (breakpoints are ascending).
        let mut color = breakpoints[0].color;
        for bp in breakpoints {
            if pct >= bp.pct {
                color = bp.color;
            } else {
                break;
            }
        }
        return color;
    }
    if pct <= breakpoints[0].pct {
        return breakpoints[0].color;
    }
    for i in 1..breakpoints.len() {
        if pct <= breakpoints[i].pct {
            let t = (pct - breakpoints[i - 1].pct) / (breakpoints[i].pct - breakpoints[i - 1].pct);
            return lerp_color(breakpoints[i - 1].color, breakpoints[i].color, t as f32);
        }
    }
    breakpoints.last().unwrap().color
}

/// Whether the context bar should use solid DOGE steps (no mid-segment lerp).
///
/// **Kind-first** for the live path: production paints with
/// `Theme::current()` which is always quantized (`Indexed` / named ANSI on
/// low-color terminals), so an RGB-only fingerprint would silently fall back
/// to lerp and invent mid-cube pastels — the failure DOGE targets.
///
/// Structural pure-primary fingerprint is a **fallback** for unit tests that
/// build `Theme::doge()` (raw or quantized) without setting the kind cache.
/// Slot contract must stay aligned with `Theme::doge()`:
/// `bg_base=black`, `text_primary=white`, `gray=white`, `warning=yellow`,
/// `accent_error=red`, `accent_assistant=magenta`, `path=cyan`.
fn uses_solid_context_steps(theme: &Theme) -> bool {
    if crate::theme::Theme::current_kind() == crate::theme::ThemeKind::Doge {
        return true;
    }
    is_doge_theme_structural(theme)
}

/// Structural DOGE pure-palette fingerprint via resolved RGB (works for pure
/// `Rgb`, pure cube `Indexed`, and named ANSI / Light* after Basic quantize).
///
/// Not the production gate — see [`uses_solid_context_steps`]. Keep in sync
/// with the semantic mapping on `Theme::doge()`.
fn is_doge_theme_structural(theme: &Theme) -> bool {
    let rgb = |c: Color| crate::render::color::resolve_to_rgb(c);
    matches!(
        (
            rgb(theme.bg_base),
            rgb(theme.text_primary),
            rgb(theme.gray),
            rgb(theme.warning),
            rgb(theme.accent_error),
            rgb(theme.accent_assistant),
            rgb(theme.path),
        ),
        (
            Some((0, 0, 0)),
            Some((255, 255, 255)),
            Some((255, 255, 255)),
            Some((255, 255, 0)),
            Some((255, 0, 0)),
            Some((255, 0, 255)),
            Some((0, 255, 255)),
        )
    )
}

/// True when `c` resolves to a DOGE pure primary (or is a named ANSI colour
/// that maps to one after resolve). Used by solid-step tests.
#[cfg(test)]
fn is_doge_primary_color(c: Color) -> bool {
    if let Some((r, g, b)) = crate::render::color::resolve_to_rgb(c) {
        return matches!(
            (r, g, b),
            (0, 0, 0)
                | (255, 0, 0)
                | (0, 255, 0)
                | (255, 255, 0)
                | (0, 0, 255)
                | (255, 0, 255)
                | (0, 255, 255)
                | (255, 255, 255)
        );
    }
    matches!(
        c,
        Color::Black
            | Color::Red
            | Color::Green
            | Color::Yellow
            | Color::Blue
            | Color::Magenta
            | Color::Cyan
            | Color::White
            | Color::LightRed
            | Color::LightGreen
            | Color::LightYellow
            | Color::LightBlue
            | Color::LightMagenta
            | Color::LightCyan
    )
}

/// Linear interpolation between two colors.
///
/// When either input is `Color::Indexed`, the result is quantized back to
/// the nearest indexed color so the output stays terminal-compatible.
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let (ar, ag, ab) = color_to_rgb(a);
    let (br, bg, bb) = color_to_rgb(b);
    let t = t.clamp(0.0, 1.0);
    let r = (ar as f32 + (br as f32 - ar as f32) * t).round() as u8;
    let g = (ag as f32 + (bg as f32 - ag as f32) * t).round() as u8;
    let b_ch = (ab as f32 + (bb as f32 - ab as f32) * t).round() as u8;
    match (a, b) {
        (Color::Indexed(_), _) | (_, Color::Indexed(_)) => {
            Color::Indexed(crate::render::color::nearest_indexed(r, g, b_ch))
        }
        _ => Color::Rgb(r, g, b_ch),
    }
}

/// RGB for any color variant, using a neutral fallback for `Reset`.
///
/// Necessary so a gradient that lerps across named breakpoints (after
/// the theme has quantized to ANSI on lower-color terminals) still
/// produces meaningful intermediate colors instead of collapsing all
/// inputs onto one fallback.
fn color_to_rgb(c: Color) -> (u8, u8, u8) {
    // (198, 198, 198) matches the FG-equivalent used elsewhere when the
    // terminal owns the actual default fg color.
    crate::render::color::resolve_to_rgb(c).unwrap_or((198, 198, 198))
}

// ---------------------------------------------------------------------------
// Status bar separator
// ---------------------------------------------------------------------------

/// The separator character between status bar items.
pub const SEPARATOR: &str = "│";

// ---------------------------------------------------------------------------
// Context bar line builder
// ---------------------------------------------------------------------------

/// Width of the percentage field on hover (`fmt_pct5` always returns 5 chars).
const PCT_WIDTH: u16 = 5;
/// Width of the gap between the progress bar and the percentage on hover.
const BAR_PCT_GAP: u16 = 1;

// BAR_BG removed — use theme.bg_highlight directly (already quantized).

/// Build the context usage bar as a `Line<'static>`.
///
/// Normal: `8.5K / 1.0M` — actual token usage, colored by the same percentage
/// gradient the hover bar uses so the urgency signal stays visible at a glance.
/// Hovered: `█████ 42.0%` — progress bar + colored percentage, sized to match.
///
/// The bar width is derived from the default token string length so the
/// hovered line has the same total width as the default (no layout shift on
/// hover). The default is right-padded to a minimum of 6 columns
/// (`BAR_PCT_GAP + PCT_WIDTH`) so the invariant holds for every input — without
/// the pad, degenerate cases like `0 / 9` (5 chars) would mismatch the hovered
/// line, which always rounds up to 6 (zero-width bar + gap + percentage).
///
/// Returns `None` if token data is unavailable.
///
/// Gateway light-frontend (`kind: "chat"`) sessions must not display Build /
/// local sampler context usage — call with `gateway_chat = true` to suppress
/// the bar entirely (remote owns context; no mapped totals yet). remote settings
/// opt-in for chat entry can reuse the same gate later.
pub fn context_bar_line(
    used_tokens: Option<u64>,
    total_tokens: Option<u64>,
    hovered: bool,
    theme: &Theme,
) -> Option<Line<'static>> {
    context_bar_line_for_session(used_tokens, total_tokens, hovered, theme, false)
}

/// Like [`context_bar_line`], but omits the bar for gateway/chat-kind sessions.
pub fn context_bar_line_for_session(
    used_tokens: Option<u64>,
    total_tokens: Option<u64>,
    hovered: bool,
    theme: &Theme,
    gateway_chat: bool,
) -> Option<Line<'static>> {
    if gateway_chat {
        return None;
    }
    let used = used_tokens?;
    let total = total_tokens.filter(|&t| t > 0)?;
    let pct = xai_token_estimation::usage_percentage(used, total);

    // Default form drives the line width: `used / total`, right-padded to the
    // minimum hover width so the two states always render at the same width.
    let mut token_str = format!("{} / {}", fmt_tokens(used), fmt_tokens(total));
    let natural_width = token_str.chars().count() as u16;
    let min_width = BAR_PCT_GAP + PCT_WIDTH;
    if natural_width < min_width {
        token_str.push_str(&" ".repeat((min_width - natural_width) as usize));
    }
    let total_width = natural_width.max(min_width);

    // Urgency color shared by both branches so the default still surfaces
    // high-usage warnings without requiring the user to hover.
    // DOGE: solid pure-palette steps only (no lerp mid-grays).
    let breakpoints = default_breakpoints(theme);
    let solid_steps = uses_solid_context_steps(theme);
    let color = crate::theme::quantize(blend_color_with_mode(pct, &breakpoints, solid_steps));

    if hovered {
        // Bar fills the space the default tokens would occupy, minus the gap
        // and the percentage. `total_width >= min_width` by construction, so
        // this subtraction is safe.
        let bar_width = total_width - min_width;
        let mut spans =
            progress_bar_spans(bar_width, pct as f32 / 100.0, color, theme.bg_highlight);
        spans.push(Span::styled(" ", Style::default().bg(theme.bg_base)));
        spans.push(Span::styled(
            fmt_pct5(pct),
            Style::default().fg(theme.text_secondary).bg(theme.bg_base),
        ));
        Some(Line::from(spans))
    } else {
        Some(Line::from(Span::styled(
            token_str,
            Style::default().fg(color).bg(theme.bg_base),
        )))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt_pct5_under_10() {
        assert_eq!(fmt_pct5(0.0), "0.00%");
        assert_eq!(fmt_pct5(5.123), "5.12%");
        assert_eq!(fmt_pct5(9.99), "9.99%");
    }

    #[test]
    fn test_fmt_pct5_10_to_99() {
        assert_eq!(fmt_pct5(10.0), "10.0%");
        assert_eq!(fmt_pct5(20.16), "20.2%"); // rounds
        assert_eq!(fmt_pct5(99.9), "99.9%");
    }

    #[test]
    fn test_fmt_pct5_max() {
        assert_eq!(fmt_pct5(100.0), "MAX %");
        assert_eq!(fmt_pct5(150.0), "MAX %");
    }

    #[test]
    fn test_fmt_pct5_all_5_chars() {
        for pct in [0.0, 0.01, 1.0, 5.55, 9.99, 10.0, 50.0, 99.9, 100.0] {
            let s = fmt_pct5(pct);
            assert_eq!(s.len(), 5, "fmt_pct5({pct}) = {s:?} should be 5 chars");
        }
    }

    #[test]
    fn test_fmt_tokens_small() {
        assert_eq!(fmt_tokens(0), "0");
        assert_eq!(fmt_tokens(12), "12");
        assert_eq!(fmt_tokens(999), "999");
    }

    #[test]
    fn test_fmt_tokens_thousands() {
        assert_eq!(fmt_tokens(1_200), "1.2K");
        assert_eq!(fmt_tokens(9_960), "10.0K"); // rounds up
        assert_eq!(fmt_tokens(9_940), "9.9K");
        assert_eq!(fmt_tokens(12_000), "12K");
        assert_eq!(fmt_tokens(123_000), "123K");
        assert_eq!(fmt_tokens(999_000), "999K");
    }

    #[test]
    fn test_fmt_tokens_millions() {
        assert_eq!(fmt_tokens(1_200_000), "1.2M");
        assert_eq!(fmt_tokens(12_000_000), "12M");
        assert_eq!(fmt_tokens(123_000_000), "123M");
    }

    #[test]
    fn test_fmt_tokens_max_4_chars() {
        for n in [
            0, 1, 999, 1_200, 9_900, 12_000, 999_000, 1_200_000, 12_000_000,
        ] {
            let s = fmt_tokens(n);
            assert!(s.len() <= 4, "fmt_tokens({n}) = {s:?} should be ≤4 chars");
        }
    }

    #[test]
    fn test_blend_color_at_breakpoints() {
        // Use unquantized theme — blend_color needs raw RGB values for lerp math.
        let theme = Theme::default();
        let bps = default_breakpoints(&theme);
        // At 0%, should be theme.text_primary
        let c0 = blend_color(0.0, &bps);
        assert_eq!(c0, theme.text_primary);
        // At 95%, should be theme.accent_error
        let c95 = blend_color(95.0, &bps);
        assert_eq!(c95, theme.accent_error);
    }

    /// Colour-distinct solid-step fixture (not DOGE white=white) so equality
    /// at 0 / 50 / 75 / 95 / 100 is meaningful.
    fn distinct_step_breakpoints() -> Vec<ColorBreakpoint> {
        vec![
            ColorBreakpoint {
                pct: 0.0,
                color: Color::Rgb(0, 255, 0), // green
            },
            ColorBreakpoint {
                pct: 50.0,
                color: Color::Rgb(255, 255, 0), // yellow
            },
            ColorBreakpoint {
                pct: 75.0,
                color: Color::Rgb(255, 0, 255), // magenta
            },
            ColorBreakpoint {
                pct: 95.0,
                color: Color::Rgb(255, 0, 0), // red
            },
        ]
    }

    #[test]
    fn solid_steps_right_closed_at_exact_breakpoints() {
        let bps = distinct_step_breakpoints();
        assert_eq!(
            blend_color_with_mode(0.0, &bps, true),
            Color::Rgb(0, 255, 0)
        );
        assert_eq!(
            blend_color_with_mode(50.0, &bps, true),
            Color::Rgb(255, 255, 0),
            "exact 50% takes the 50% breakpoint colour"
        );
        assert_eq!(
            blend_color_with_mode(75.0, &bps, true),
            Color::Rgb(255, 0, 255)
        );
        assert_eq!(
            blend_color_with_mode(95.0, &bps, true),
            Color::Rgb(255, 0, 0),
            "exact 95% is error red, not the prior yellow/magenta hold"
        );
        assert_eq!(
            blend_color_with_mode(100.0, &bps, true),
            Color::Rgb(255, 0, 0)
        );
        // Just below a threshold holds the previous step.
        assert_eq!(
            blend_color_with_mode(94.9, &bps, true),
            Color::Rgb(255, 0, 255)
        );
        assert_eq!(
            blend_color_with_mode(95.1, &bps, true),
            Color::Rgb(255, 0, 0)
        );
        // Mid-segment holds lower (no lerp pastels).
        assert_eq!(
            blend_color_with_mode(60.0, &bps, true),
            Color::Rgb(255, 255, 0)
        );
    }

    #[test]
    fn doge_context_bar_solid_steps_at_0_50_100() {
        let theme = Theme::doge();
        assert!(uses_solid_context_steps(&theme));
        let bps = default_breakpoints(&theme);

        // Right-closed: 0% white, 50% accent_user (also white on DOGE),
        // 95%+ error red, 100% error red.
        let c0 = blend_color_with_mode(0.0, &bps, true);
        let c50 = blend_color_with_mode(50.0, &bps, true);
        let c95 = blend_color_with_mode(95.0, &bps, true);
        let c100 = blend_color_with_mode(100.0, &bps, true);
        assert_eq!(c0, theme.text_primary, "0% step");
        assert_eq!(c50, theme.accent_user, "50% step (right-closed)");
        assert_eq!(c95, theme.accent_error, "exact 95% is error");
        assert_eq!(c100, theme.accent_error, "100% holds last breakpoint");
        assert!(is_doge_primary_color(c0));
        assert!(is_doge_primary_color(c50));
        assert!(is_doge_primary_color(c95));
        assert!(is_doge_primary_color(c100));
    }

    #[test]
    fn doge_context_bar_mid_segment_holds_lower_not_lerp_gray() {
        let theme = Theme::doge();
        let bps = default_breakpoints(&theme);
        // Between 65% (accent_user/white) and 75% (warning/yellow): solid
        // holds white. Lerp would invent pale yellow / off-palette midtones.
        let c70 = blend_color_with_mode(70.0, &bps, true);
        assert_eq!(c70, theme.accent_user);
        assert!(is_doge_primary_color(c70));
        // Between 85% (warning) and 95% (error): hold yellow; exact 95 is red.
        let c90 = blend_color_with_mode(90.0, &bps, true);
        assert_eq!(c90, theme.warning);
        assert!(is_doge_primary_color(c90));
        assert_eq!(blend_color_with_mode(95.0, &bps, true), theme.accent_error);
    }

    #[test]
    fn doge_context_bar_line_usage_colors_are_doge_only() {
        let theme = Theme::doge();
        // 0%, 50%, 100% of a 1M window — solid-step path before quantize.
        // (quantize may map pure white → Reset under some color levels; that
        // is still not an off-palette mid-gray.)
        for (used, total, expect_step) in [
            (0u64, 1_000_000u64, theme.text_primary),
            (500_000, 1_000_000, theme.accent_user),
            (1_000_000, 1_000_000, theme.accent_error),
        ] {
            let pct = xai_token_estimation::usage_percentage(used, total);
            let bps = default_breakpoints(&theme);
            let solid = blend_color_with_mode(pct, &bps, true);
            assert_eq!(solid, expect_step, "usage {used}/{total} step");
            assert!(is_doge_primary_color(solid));
            // Live path must still produce a line (and not invent mid-gray RGB).
            let line =
                context_bar_line(Some(used), Some(total), false, &theme).expect("token data");
            for span in &line.spans {
                if let Some(fg) = span.style.fg {
                    if matches!(fg, Color::Reset) {
                        continue;
                    }
                    assert!(
                        is_doge_primary_color(fg),
                        "usage {used}/{total} painted off-palette {fg:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn doge_quantized_themes_still_use_solid_steps() {
        // Production path: Theme::current() is always quantized. Solid steps
        // must not depend on raw Color::Rgb fingerprints alone.
        use crate::theme::color_support::ColorLevel;
        use crate::theme::{ThemeKind, cache as theme_cache};

        let _guard = theme_cache::test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        theme_cache::reset_for_test();
        theme_cache::set(ThemeKind::Doge);
        assert_eq!(Theme::current_kind(), ThemeKind::Doge);

        for level in [
            ColorLevel::TrueColor,
            ColorLevel::Ansi256,
            ColorLevel::Basic,
        ] {
            let theme = Theme::doge().quantized(level);
            assert!(
                uses_solid_context_steps(&theme),
                "kind=Doge must enable solid steps after {level:?} quantize"
            );
            // Structural fingerprint also survives pure-primary quantize
            // (defense in depth when kind is set — and for kind-less tests).
            if level != ColorLevel::None {
                assert!(
                    is_doge_theme_structural(&theme),
                    "structural DOGE fingerprint should hold after {level:?}"
                );
            }

            let bps = default_breakpoints(&theme);
            // Mid white→yellow segment must not lerp to a non-primary.
            let c70 = blend_color_with_mode(70.0, &bps, true);
            assert!(
                is_doge_primary_color(c70),
                "70% solid step off-palette after {level:?}: {c70:?}"
            );
            // Live line path: no mid-tone RGB/Indexed pastels in span fg.
            let line = context_bar_line(Some(700_000), Some(1_000_000), false, &theme)
                .expect("token data");
            for span in &line.spans {
                if let Some(fg) = span.style.fg {
                    if matches!(fg, Color::Reset) {
                        continue;
                    }
                    // After a second global quantize the fg may be Indexed;
                    // resolve and require pure DOGE RGB.
                    if let Some((r, g, b)) = crate::render::color::resolve_to_rgb(fg) {
                        assert!(
                            is_doge_primary_color(Color::Rgb(r, g, b)),
                            "quantized live path {level:?} painted mid-tone {fg:?} → ({r},{g},{b})"
                        );
                    }
                }
            }
        }

        // Without kind cache, quantized structural still enables solid steps.
        theme_cache::set(ThemeKind::GrokNight);
        let q256 = Theme::doge().quantized(ColorLevel::Ansi256);
        assert!(
            uses_solid_context_steps(&q256),
            "quantized pure-primary theme must still solid-step without Doge kind"
        );
        theme_cache::reset_for_test();
    }

    #[test]
    fn non_doge_theme_still_lerps_between_breakpoints() {
        // Default (GrokNight) must keep the smooth gradient between
        // neighbouring breakpoints — solid-steps is DOGE-only.
        // Hold the theme test lock so a parallel Doge kind set cannot
        // flip uses_solid_context_steps via current_kind().
        use crate::theme::{ThemeKind, cache as theme_cache};
        let _guard = theme_cache::test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        theme_cache::reset_for_test();
        theme_cache::set(ThemeKind::GrokNight);

        let theme = Theme::default();
        assert!(!uses_solid_context_steps(&theme));
        assert!(!is_doge_theme_structural(&theme));
        let bps = default_breakpoints(&theme);
        // 70% sits between 65% (accent_user) and 75% (warning).
        let solid = blend_color_with_mode(70.0, &bps, true);
        let lerped = blend_color_with_mode(70.0, &bps, false);
        assert_eq!(solid, theme.accent_user);
        // Lerp should differ from either endpoint when the endpoints differ.
        if theme.accent_user != theme.warning {
            assert_ne!(
                lerped, theme.accent_user,
                "expected mid-segment lerp, not solid hold"
            );
            assert_ne!(lerped, theme.warning);
        }
        theme_cache::reset_for_test();
    }

    /// Concatenate all span content into one string for assertions.
    fn line_text(line: &Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_context_bar_default_shows_compact_token_usage() {
        // Default (non-hovered) state shows `used / total` with no padding.
        let theme = Theme::default();
        let line = context_bar_line(Some(8_500), Some(1_000_000), false, &theme)
            .expect("token data provided");
        let text = line_text(&line);
        assert_eq!(text, "8.5K / 1.0M");
    }

    #[test]
    fn test_context_bar_hover_shows_bar_and_percentage() {
        // Hovered state shows the progress bar followed by the percentage.
        let theme = Theme::default();
        let line =
            context_bar_line(Some(420_000), Some(1_000_000), true, &theme).expect("token data");
        let text = line_text(&line);
        assert!(
            text.ends_with("42.0%"),
            "expected hovered line to end with '42.0%', got: {text:?}"
        );
    }

    #[test]
    fn test_context_bar_hover_width_matches_default() {
        // For each (used, total) combo, the hovered line must be the same
        // width as the default — toggling hover should never shift layout.
        let theme = Theme::default();
        for (used, total) in [
            (8_500u64, 1_000_000u64),
            (500, 1_000_000),
            (123_456, 1_000_000),
            (999_999, 999_999),
            (12_000_000, 12_000_000),
            // Degenerate sub-min-width case: default natural width is 5
            // ("0 / 9"), padded to 6 so the hover line still matches.
            (0, 9),
        ] {
            let default_line = context_bar_line(Some(used), Some(total), false, &theme)
                .expect("token data provided");
            let hover_line = context_bar_line(Some(used), Some(total), true, &theme)
                .expect("token data provided");
            assert_eq!(
                default_line.width(),
                hover_line.width(),
                "default vs hover width mismatch for used={used} total={total}: \
                 default={:?} hover={:?}",
                line_text(&default_line),
                line_text(&hover_line),
            );
        }
    }

    #[test]
    fn test_context_bar_hover_bar_grows_with_token_string() {
        // The bar size should scale with the default string length.
        // `500 / 1.0M` (10 chars) → bar = 10 - 6 = 4 chars.
        // `8.5K / 1.0M` (11 chars) → bar = 11 - 6 = 5 chars.
        let theme = Theme::default();
        let short = context_bar_line(Some(500), Some(1_000_000), true, &theme).unwrap();
        let long = context_bar_line(Some(8_500), Some(1_000_000), true, &theme).unwrap();
        assert!(
            short.width() < long.width(),
            "expected shorter default to produce shorter hover line; \
             short={:?} ({} cols), long={:?} ({} cols)",
            line_text(&short),
            short.width(),
            line_text(&long),
            long.width(),
        );
    }

    #[test]
    fn test_context_bar_returns_none_without_tokens() {
        // Mirror across hover states so a future refactor that moves the
        // unavailability checks into per-branch arms can't silently regress
        // one path.
        let theme = Theme::default();
        for hovered in [false, true] {
            assert!(context_bar_line(None, Some(1_000_000), hovered, &theme).is_none());
            assert!(context_bar_line(Some(1_000), None, hovered, &theme).is_none());
            // Zero total is treated as missing.
            assert!(context_bar_line(Some(1_000), Some(0), hovered, &theme).is_none());
        }
    }

    #[test]
    fn gateway_chat_suppresses_context_bar_even_with_tokens() {
        let theme = Theme::default();
        assert!(
            context_bar_line_for_session(Some(1_000), Some(1_000_000), false, &theme, true)
                .is_none()
        );
        assert!(
            context_bar_line_for_session(Some(1_000), Some(1_000_000), false, &theme, false)
                .is_some()
        );
    }
}
