//! Legacy-console fallbacks for chrome glyphs that don't ship in the legacy Windows ConHost default font (Consolas / Lucida Console).
//!
//! Fallbacks are ASCII where possible (`x`, `o`, `c`, `*`), or a CP437 glyph when one reads better and still renders on the raster font.
//!
//! ConHost does no font fallback, so missing glyphs render as tofu.
//! Windows Terminal, VS Code, and modern emulators bundle fonts (or fall back to one) that cover the glyphs we use as chrome.
//! The substitution therefore only fires for legacy `cmd.exe` / `powershell.exe`.

use std::borrow::Cow;
use std::sync::OnceLock;

use crate::host::HostOs;
use crate::terminal::{TerminalName, terminal_context};

/// `"❯ "` normally, `"> "` on legacy ConHost. Always 2 columns wide.
pub fn prompt_arrow() -> &'static str {
    if is_legacy_windows_console() {
        "> "
    } else {
        "\u{276F} "
    }
}

/// Display width of [`prompt_arrow`] in columns.
pub const PROMPT_ARROW_WIDTH: u16 = 2;

/// Voice-capture pulse: filled vs open ring, with a 1-column ASCII fallback on legacy ConHost.
pub fn record_dot(filled: bool) -> &'static str {
    if is_legacy_windows_console() {
        if filled { "*" } else { "o" }
    } else if filled {
        "\u{25C9}"
    } else {
        "\u{25CE}"
    }
}

/// `"❙"` normally, `"|"` on legacy ConHost. Always 1 column wide.
pub fn collapsed_accent() -> &'static str {
    if is_legacy_windows_console() {
        "|"
    } else {
        "\u{2759}"
    }
}

/// `"✗"` (U+2717 BALLOT X) normally, `"x"` on legacy ConHost. Always 1 column wide.
///
/// Used for close / cancel / kill buttons and failure status markers.
pub fn ballot_x() -> &'static str {
    if is_legacy_windows_console() {
        "x"
    } else {
        "\u{2717}"
    }
}

/// Check mark; legacy ConHost uses CP437 `√` so the raster font still reads as done. Always 1 column.
pub fn check_mark() -> &'static str {
    if is_legacy_windows_console() {
        "\u{221A}"
    } else {
        "\u{2713}"
    }
}

/// Enlarge glyph. U+26F6 is tofu in many monospace fonts; U+2197 is in the core Arrows block. Legacy ConHost uses `o`.
pub fn enlarge() -> &'static str {
    if is_legacy_windows_console() {
        "o"
    } else {
        "\u{2197}"
    }
}

/// `"⧉"` (U+29C9 TWO JOINED SQUARES) normally, `"c"` on legacy ConHost. Always 1 column wide.
///
/// The copy button glyph on the scrollback selection box (pairs with [`enlarge`]).
pub fn copy_icon() -> &'static str {
    if is_legacy_windows_console() {
        "c"
    } else {
        "\u{29C9}"
    }
}

/// Magnifying glass on the Isolated Preview title bar, immediately left of copy.
/// U+2315 normally, `"s"` on legacy ConHost. Always 1 column.
pub fn search_icon() -> &'static str {
    if is_legacy_windows_console() {
        "s"
    } else {
        "\u{2315}"
    }
}

/// How long one nested-overlay sparkler frame stays up.
/// Wide enough that a single draw stays inside the frame the elapsed clock asked for.
pub const SPARKLER_FRAME_MS: u64 = 2_000;

/// Live-work glyph at `elapsed_ms` on the nested overlay title.
/// Non-DOGE themes use the dot spinner. DOGE uses the striped downward marquee.
/// Frame 0 is the start of the clock. One [`SPARKLER_FRAME_MS`] dwell advances one frame.
pub fn sparkler_frame_at_ms(elapsed_ms: u64) -> &'static str {
    let frames = if prefers_doge_striped_spinners() {
        if is_legacy_windows_console() {
            doge_striped_down_frames_ascii()
        } else {
            doge_striped_down_frames()
        }
    } else {
        dot_spinner_frames()
    };
    if frames.is_empty() {
        return "";
    }
    let step = SPARKLER_FRAME_MS.max(1);
    let idx = ((elapsed_ms / step) as usize) % frames.len();
    frames.get(idx).copied().unwrap_or("")
}

/// `"⇣"` (U+21E3 DOWNWARDS DASHED ARROW) normally, `"↓"` (U+2193) on legacy ConHost. Always 1 column wide.
///
/// Used for the context-token count in the turn-status line.
pub fn token_arrow() -> &'static str {
    if is_legacy_windows_console() {
        "\u{2193}"
    } else {
        "\u{21E3}"
    }
}

/// Monitor-running pulse. Only `○` is CP437, so legacy ConHost pulses by fill, not size.
/// Every frame is 1 column so the trailing label does not shift.
pub fn monitor_icon_frames() -> &'static [&'static str] {
    const FANCY: &[&str] = &["\u{25CB}", "\u{25CE}", "\u{25C9}", "\u{25CE}"];
    const FALLBACK: &[&str] = &["\u{00B7}", "\u{25CB}", "\u{2022}", "\u{25CB}"];
    if is_legacy_windows_console() {
        FALLBACK
    } else {
        FANCY
    }
}

/// Filled diamond; legacy ConHost uses CP437 `♦` so the raster font still renders. Always 1 column.
pub fn diamond_filled() -> &'static str {
    if is_legacy_windows_console() {
        "\u{2666}"
    } else {
        "\u{25C6}"
    }
}

/// Hollow diamond for unused/idle markers; legacy ConHost uses CP437 `○`. Always 1 column.
pub fn diamond_hollow() -> &'static str {
    if is_legacy_windows_console() {
        "\u{25CB}"
    } else {
        "\u{25C7}"
    }
}

/// Dotted diamond; legacy ConHost shares [`diamond_filled`]'s `♦` because call sites already distinguish by color.
pub fn diamond_dotted() -> &'static str {
    if is_legacy_windows_console() {
        "\u{2666}"
    } else {
        "\u{25C8}"
    }
}

/// Filled-diamond glyph as a [`char`] (see [`diamond_filled`]), for the tool-usage sequence bar which builds its row from single `char`s.
pub fn diamond_filled_char() -> char {
    diamond_filled().chars().next().unwrap_or('\u{25C6}')
}

/// Hollow-diamond glyph as a [`char`] (see [`diamond_hollow`]).
pub fn diamond_hollow_char() -> char {
    diamond_hollow().chars().next().unwrap_or('\u{25C7}')
}

/// Striped downward marquee used as the DOGE activity spinner.
pub fn doge_striped_down_frames() -> &'static [&'static str] {
    const FRAMES: &[&str] = &[
        "\u{2503}", "\u{2507}", "\u{250b}", "\u{250a}", "\u{2502}", "\u{2506}", "\u{00b7}",
        "\u{2577}",
    ];
    FRAMES
}

fn doge_striped_down_frames_ascii() -> &'static [&'static str] {
    const FRAMES: &[&str] = &["|", "!", ":", ".", ":", "!", "|", "."];
    FRAMES
}

fn prefers_doge_striped_spinners() -> bool {
    matches!(
        crate::theme::Theme::current_kind(),
        crate::theme::ThemeKind::Doge
    )
}

/// Braille spinner; U+2800 is not CP437, so legacy ConHost uses a 1-column ASCII spinner. Frames stay 1 column so layout does not shift.
pub fn braille_spinner_frames() -> &'static [&'static str] {
    const FANCY: &[&str] = &[
        "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}",
        "\u{2827}",
    ];
    const FALLBACK: &[&str] = &["|", "/", "-", "\\"];
    if prefers_doge_striped_spinners() {
        if is_legacy_windows_console() {
            doge_striped_down_frames_ascii()
        } else {
            doge_striped_down_frames()
        }
    } else if is_legacy_windows_console() {
        FALLBACK
    } else {
        FANCY
    }
}

/// Dot spinner; those code points are absent from CP437, so legacy ConHost uses a 1-column dot cycle.
pub fn dot_spinner_frames() -> &'static [&'static str] {
    const FANCY: &[&str] = &[
        "\u{22c5}", ":", "\u{2e2c}", "\u{2059}", "\u{22c5}", ":", "\u{2e2c}", "\u{2059}",
    ];
    const FALLBACK: &[&str] = &[".", ":", "\u{00b7}"];
    if is_legacy_windows_console() {
        FALLBACK
    } else {
        FANCY
    }
}

/// Accent rail. CP437 has no heavy vertical, so legacy ConHost uses light `│`. Always 1 column.
pub fn accent_bar() -> &'static str {
    if is_legacy_windows_console() {
        "\u{2502}"
    } else {
        "\u{2503}"
    }
}

/// Composer caret: **solid** half of the classic full-cell block blink.
///
/// Paired with [`cursor_box_hollow`] as one **box/block caret family**: solid
/// filled cell ↔ visible off-half of the **same terminal-cell silhouette**.
/// Outer height is the cell itself via the background plate on the solid
/// half — not outline glyph metrics. Live blank-insertion paint keeps this
/// glyph on the off-half (Human-green ink, canvas bg) so the caret does not
/// vanish.
///
/// Glyph (paint pairs with `fg = bg = accent` solid plate):
/// - `█` U+2588 FULL BLOCK (classic solid block ink on the plate).
/// - Legacy ConHost: `#`.
///
/// Rejected mates for the *empty* half: canvas hole-punch on an accent plate
/// (`■` with `fg=canvas bg=accent` — reads as a green tile with a void),
/// dimming the solid `█`, skinny `▯`, medium `◼`/`◻`, tiny mid-cell `□`,
/// and short outline quads (`⎕` / `□`) whose ink is only a fraction of the
/// cell in common monospace (e.g. Noto Sans Mono).
pub fn cursor_box_filled() -> &'static str {
    if is_legacy_windows_console() {
        "#"
    } else {
        "\u{2588}" // █ FULL BLOCK
    }
}

/// Composer caret: **empty** half helper (plain space).
///
/// The blank-insertion **paint** path does not use this glyph for the live
/// off-half: it keeps [`cursor_box_filled`] with Human-green ink on canvas
/// so an empty Human box still shows a caret. This helper stays a space
/// for phase tests and for `cursor_box_glyph`.
///
/// Rejected empty shapes: `■` hole-punch on accent plate, dim `█`, skinny
/// `▯`, medium `◻`/`◼`, tiny `□`, short APL quad `⎕`, and a true empty
/// cell (space + canvas plate) that vanished until the next key.
pub fn cursor_box_hollow() -> &'static str {
    // Classic block empty half: plain space. Paint path must not put an
    // accent background plate behind this glyph.
    " "
}

/// Half-period for the composer filled↔empty block blink (milliseconds).
/// ~600ms keeps the blink slow and readable (not seizure-fast).
pub const CURSOR_BOX_BLINK_HALF_MS: u64 = 600;

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static CURSOR_BOX_FILLED_PHASE_PIN: std::cell::Cell<Option<bool>> =
        const { std::cell::Cell::new(None) };
}

/// Guard from [`pin_cursor_box_filled_phase`]. Drop restores the prior pin.
#[cfg(any(test, feature = "test-support"))]
pub struct CursorBoxFilledPhasePin {
    prev: Option<bool>,
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for CursorBoxFilledPhasePin {
    fn drop(&mut self) {
        CURSOR_BOX_FILLED_PHASE_PIN.with(|c| c.set(self.prev));
    }
}

/// Force the composer box caret into the filled or empty blink half for
/// this thread. Production paint still uses wall-clock [`cursor_box_filled_phase`];
/// tests must not sleep hoping the clock lands in the solid half.
#[cfg(any(test, feature = "test-support"))]
pub fn pin_cursor_box_filled_phase(filled: bool) -> CursorBoxFilledPhasePin {
    let prev = CURSOR_BOX_FILLED_PHASE_PIN.with(|c| c.replace(Some(filled)));
    CursorBoxFilledPhasePin { prev }
}

/// Whether the filled (solid plate) phase is showing at `now_ms` (unix millis
/// in production; tests may pin a phase via [`pin_cursor_box_filled_phase`]).
pub fn cursor_box_filled_phase(now_ms: u64) -> bool {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(pinned) = CURSOR_BOX_FILLED_PHASE_PIN.with(std::cell::Cell::get) {
        return pinned;
    }
    (now_ms / CURSOR_BOX_BLINK_HALF_MS).is_multiple_of(2)
}

/// Glyph for the composer caret at `now_ms` (filled `█` or empty space).
pub fn cursor_box_glyph(now_ms: u64) -> &'static str {
    if cursor_box_filled_phase(now_ms) {
        cursor_box_filled()
    } else {
        cursor_box_hollow()
    }
}

/// Timeline up-chevron. Small triangles are absent from CP437; legacy ConHost uses full-size `▲`.
pub fn timeline_chevron_up() -> &'static str {
    if is_legacy_windows_console() {
        "\u{25B2}"
    } else {
        "\u{25B4}"
    }
}

/// `"▾"` (U+25BE SMALL DOWN-POINTING TRIANGLE) normally, `"▼"` (U+25BC, CP437 `0x1F`) on legacy ConHost. Always 1 column wide.
///
/// The timeline sidebar's next-turn chevron; see [`timeline_chevron_up`].
pub fn timeline_chevron_down() -> &'static str {
    if is_legacy_windows_console() {
        "\u{25BC}"
    } else {
        "\u{25BE}"
    }
}

/// `"━"` (U+2501 HEAVY HORIZONTAL) normally, `"─"` (U+2500 LIGHT HORIZONTAL, CP437 `0xC4`) on legacy ConHost. Always 1 column wide.
///
/// Prefer [`timeline_tick_active`] for the sidebar rail: on legacy ConHost this falls back to the same light stroke used for hover.
pub fn heavy_horizontal() -> &'static str {
    if is_legacy_windows_console() {
        "\u{2500}"
    } else {
        "\u{2501}"
    }
}

/// `"─"` (U+2500 LIGHT HORIZONTAL, CP437 `0xC4`). Always 1 column wide and present on every target.
/// Exposed so the timeline sidebar's inactive ticks share one glyph source with [`heavy_horizontal`] instead of hardcoding the codepoint.
pub fn light_horizontal() -> &'static str {
    "\u{2500}"
}

/// Precomposed 2-col active tick for the timeline rail: `"━━"` normally, `"══"` (U+2550, CP437 `0xCD`) on legacy ConHost.
/// The double stroke keeps the active tick distinct from the light hover/idle stroke there.
pub fn timeline_tick_active() -> &'static str {
    if is_legacy_windows_console() {
        "\u{2550}\u{2550}"
    } else {
        "\u{2501}\u{2501}"
    }
}

/// Precomposed 2-col hover tick for the timeline rail: `"──"` (light horizontal).
/// Idle ticks reuse a single light cell; this is the wide bright hover form.
pub fn timeline_tick_hover() -> &'static str {
    "\u{2500}\u{2500}"
}

/// Filled status dot. Hollow `○` is already CP437; only the filled form needs a `•` stand-in on legacy ConHost.
pub fn filled_dot() -> &'static str {
    if is_legacy_windows_console() {
        "\u{2022}"
    } else {
        "\u{25CF}"
    }
}

/// `"▏"` (U+258F LEFT ONE EIGHTH BLOCK) normally, `"│"` (U+2502, CP437 `0xB3`) on legacy ConHost. Always 1 column wide.
///
/// The thin left bar marking the selected row in the dashboard and the settings panes.
pub fn selection_bar() -> &'static str {
    if is_legacy_windows_console() {
        "\u{2502}"
    } else {
        "\u{258F}"
    }
}

/// `"›"` (U+203A SINGLE RIGHT-POINTING ANGLE QUOTATION MARK) normally, `">"` (ASCII) on legacy ConHost. Always 1 column wide.
///
/// The chevron used for collapsed fold indicators, settings breadcrumbs, the integer-stepper increment affordance, and the dashboard "next" button.
pub fn chevron() -> &'static str {
    if is_legacy_windows_console() {
        ">"
    } else {
        "\u{203A}"
    }
}

/// Left chevron, kept in lockstep with [`chevron`] so a fixed `>` never sits next to tofu `‹` on legacy ConHost.
pub fn chevron_left() -> &'static str {
    if is_legacy_windows_console() {
        "<"
    } else {
        "\u{2039}"
    }
}

/// Down chevron matching `›`'s light weight (not solid `▾`). Legacy ConHost uses `v`.
pub fn chevron_down() -> &'static str {
    if is_legacy_windows_console() {
        "v"
    } else {
        "\u{2304}"
    }
}

/// `"▾"` (U+25BE BLACK DOWN-POINTING SMALL TRIANGLE) normally, `"v"` (ASCII) on legacy ConHost. Always 1 column wide.
///
/// The "expanded" disclosure indicator for a collapsible dashboard section header (the section's rows are visible below it).
pub fn disclosure_open() -> &'static str {
    if is_legacy_windows_console() {
        "v"
    } else {
        "\u{25BE}"
    }
}

/// Collapsed disclosure; pairs with [`disclosure_open`]. Legacy ConHost uses `>`.
pub fn disclosure_closed() -> &'static str {
    if is_legacy_windows_console() {
        ">"
    } else {
        "\u{25B8}"
    }
}

/// `"▴"` (U+25B4 BLACK UP-POINTING SMALL TRIANGLE) normally, `"^"` (ASCII) on legacy ConHost. Always 1 column wide.
pub fn disclosure_up() -> &'static str {
    if is_legacy_windows_console() {
        "^"
    } else {
        "\u{25B4}"
    }
}

/// `"[✗]"` normally, `"[x]"` on legacy ConHost. Always 3 columns wide.
///
/// Pre-composed bracketed form of [`ballot_x`] so per-frame render paths reuse a `&'static str` instead of allocating a `format!` each frame.
pub fn ballot_x_button() -> &'static str {
    if is_legacy_windows_console() {
        "[x]"
    } else {
        "[\u{2717}]"
    }
}

/// `"[\u{2212}]"` normally, `"[-]"` on legacy ConHost. Always 3 columns wide.
///
/// Clear-finished control. Minus, not ballot X.
pub fn clear_finished_button() -> &'static str {
    if is_legacy_windows_console() {
        "[-]"
    } else {
        "[\u{2212}]"
    }
}

/// `"[↗]"` normally, `"[o]"` on legacy ConHost. Always 3 columns wide.
///
/// Pre-composed bracketed sibling of [`ballot_x_button`] for the bg-task view / enlarge button.
pub fn enlarge_button() -> &'static str {
    if is_legacy_windows_console() {
        "[o]"
    } else {
        "[\u{2197}]"
    }
}

/// One funnel for toast chrome that legacy ConHost cannot render. Non-legacy platforms return the borrow unchanged.
pub fn legacy_glyph_fallback(s: &str) -> Cow<'_, str> {
    if !is_legacy_windows_console() {
        return Cow::Borrowed(s);
    }
    if !s.contains(['\u{2713}', '\u{2717}', '\u{26A0}']) {
        return Cow::Borrowed(s);
    }
    Cow::Owned(to_legacy_glyphs(s))
}

/// Single-row toast sinks: glyph fallback, then map control chars to spaces.
/// Borrows when the input is already clean (common path).
pub fn sanitize_toast_message(msg: &str) -> Cow<'_, str> {
    let glyph = legacy_glyph_fallback(msg);
    if !glyph.chars().any(char::is_control) {
        return glyph;
    }
    Cow::Owned(
        glyph
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect(),
    )
}

/// Pure glyph-to-legacy mapping behind [`legacy_glyph_fallback`], split out so tests can exercise the substitution without faking the host probe.
/// `√` matches [`check_mark`]'s fallback; `x` matches [`ballot_x`]'s.
fn to_legacy_glyphs(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{2713}' => '\u{221A}',
            '\u{2717}' => 'x',
            '\u{26A0}' => '!',
            other => other,
        })
        .collect()
}

/// Cached: native Windows console whose font lacks our Dingbats chrome.
/// `GROK_FORCE_LEGACY_CONSOLE` overrides so QA can check fallbacks without ConHost.
pub fn is_legacy_windows_console() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| {
        forced_legacy_console_override().unwrap_or_else(|| {
            // `env_brand`, not `brand`: bare ConHost detects as `Unknown`, but `brand` optimistically becomes `WindowsTerminal` on native Windows
            // Font capability needs the raw detection so legacy consoles still get the ASCII glyph fallback
            decide_legacy_windows_console(HostOs::current(), terminal_context().env_brand)
        })
    })
}

/// Read the `GROK_FORCE_LEGACY_CONSOLE` escape hatch from the environment.
fn forced_legacy_console_override() -> Option<bool> {
    parse_forced_legacy_console(std::env::var("GROK_FORCE_LEGACY_CONSOLE").ok().as_deref())
}

/// Pure parse of the override value so tests don't touch the environment.
/// `"1"` / `"true"` forces on, `"0"` / `"false"` forces off; anything else (including unset) is `None` so normal host/brand detection runs.
fn parse_forced_legacy_console(value: Option<&str>) -> Option<bool> {
    match value {
        Some("1" | "true") => Some(true),
        Some("0" | "false") => Some(false),
        _ => None,
    }
}

/// Pure decision function so tests can drive (host, brand) pairs without touching ambient state.
/// Default-deny on Windows: an unknown brand is treated as legacy.
/// Bare `cmd.exe` / `powershell.exe` in ConHost sets no terminal env vars, so the brand probe returns `Unknown` in exactly the case we need to catch.
fn decide_legacy_windows_console(host: HostOs, brand: TerminalName) -> bool {
    if host != HostOs::Windows {
        return false;
    }
    !matches!(
        brand,
        TerminalName::WindowsTerminal
            | TerminalName::VsCode
            | TerminalName::Cursor
            | TerminalName::Windsurf
            | TerminalName::Zed
            | TerminalName::WezTerm
            | TerminalName::Kitty
            | TerminalName::Alacritty
            | TerminalName::Ghostty
            | TerminalName::Rio
            | TerminalName::GrokDesktop
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    // Both variants must match `PROMPT_ARROW_WIDTH` so callers using the constant for layout math don't drift between platforms
    #[test]
    fn prompt_arrow_variants_are_two_columns() {
        assert_eq!("\u{276F} ".width(), PROMPT_ARROW_WIDTH as usize);
        assert_eq!("> ".width(), PROMPT_ARROW_WIDTH as usize);
    }

    // Both record-dot states must be exactly 1 column so the "Recording" label position is stable as the indicator pulses
    #[test]
    fn record_dot_states_are_one_column() {
        assert_eq!(record_dot(true).width(), 1);
        assert_eq!(record_dot(false).width(), 1);
        assert_eq!("\u{25C9}".width(), 1); // ◉ FISHEYE
        assert_eq!("\u{25CE}".width(), 1); // ◎ BULLSEYE
    }

    #[test]
    fn collapsed_accent_variants_are_one_column() {
        assert_eq!("\u{2759}".width(), 1);
        assert_eq!("|".width(), 1);
    }

    // Every icon and its fallback must be exactly one column so fixed-width button layouts don't shift between platforms
    #[test]
    fn icon_fallback_variants_are_one_column() {
        for (fancy, fallback) in [
            ("\u{2717}", "x"),        // ballot_x
            ("\u{2713}", "\u{221A}"), // check_mark
            ("\u{2197}", "o"),        // enlarge
            ("\u{29C9}", "c"),        // copy_icon
            ("\u{2315}", "s"),        // search_icon
            ("\u{21E3}", "\u{2193}"), // token_arrow
        ] {
            assert_eq!(fancy.width(), 1, "icon {fancy:?} must be 1 column");
            assert_eq!(
                fallback.width(),
                1,
                "fallback {fallback:?} must be 1 column"
            );
        }
        // Live helper (legacy or fancy) stays 1 col so fixed `[⧉]` hit widths hold.
        assert_eq!(
            copy_icon().width(),
            1,
            "copy_icon() must stay 1 column wide"
        );
    }

    // Every diamond glyph and its legacy fallback must be exactly one column so the call sites keep their layout on every platform
    #[test]
    fn diamond_variants_are_one_column() {
        for (fancy, fallback) in [
            ("\u{25C6}", "\u{2666}"), // diamond_filled
            ("\u{25C7}", "\u{25CB}"), // diamond_hollow
            ("\u{25C8}", "\u{2666}"), // diamond_dotted
        ] {
            assert_eq!(fancy.width(), 1, "diamond {fancy:?} must be 1 column");
            assert_eq!(
                fallback.width(),
                1,
                "fallback {fallback:?} must be 1 column"
            );
        }
    }

    // Each chrome glyph and its legacy fallback must be exactly one column so the rails, dots, bars, and chevrons keep their layout everywhere
    #[test]
    fn chrome_glyph_variants_are_one_column() {
        for (fancy, fallback) in [
            ("\u{2503}", "\u{2502}"), // accent_bar
            ("\u{25CF}", "\u{2022}"), // filled_dot
            ("\u{258F}", "\u{2502}"), // selection_bar
            ("\u{203A}", ">"),        // chevron
            ("\u{2039}", "<"),        // chevron_left
            ("\u{2304}", "v"),        // chevron_down
        ] {
            assert_eq!(fancy.width(), 1, "glyph {fancy:?} must be 1 column");
            assert_eq!(
                fallback.width(),
                1,
                "fallback {fallback:?} must be 1 column"
            );
        }
    }

    // Every spinner and monitor-pulse frame, fancy or fallback, must be 1 column so animating them never shifts the label / timer that follows
    #[test]
    fn spinner_frames_are_one_column() {
        for frame in braille_spinner_frames()
            .iter()
            .chain(dot_spinner_frames().iter())
            .chain(doge_striped_down_frames().iter())
            .chain(doge_striped_down_frames_ascii().iter())
            .chain(monitor_icon_frames().iter())
            .chain(
                [
                    "|", "/", "-", "\\", ".", ":", "\u{00b7}", "\u{25cb}", "\u{2022}",
                ]
                .iter(),
            )
        {
            assert_eq!(frame.width(), 1, "spinner frame {frame:?} must be 1 column");
        }
    }

    /// DOGE activity spinner is the striped downward marquee, not braille.
    #[test]
    fn doge_activity_spinners_use_striped_down_marquee_not_braille() {
        let _pin = crate::theme::cache::pin_theme();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);
        let striped = doge_striped_down_frames();
        assert!(striped.len() >= 6);
        assert_eq!(braille_spinner_frames(), striped);
        assert_ne!(striped.first().copied(), Some("\u{280b}"));
        assert!(striped.contains(&"\u{2507}"));
        assert!(striped.contains(&"\u{250b}"));
        assert!(striped.contains(&"\u{250a}"));
        assert_eq!(dot_spinner_frames().get(2).copied(), Some("\u{2e2c}"));
        assert_ne!(braille_spinner_frames(), dot_spinner_frames());
    }

    /// Composer box cursors are 1 column.
    #[test]
    fn cursor_box_glyphs_are_one_column() {
        assert_eq!(cursor_box_filled().width(), 1);
        assert_eq!(cursor_box_hollow().width(), 1);
        // Solid block vs empty space are distinct glyphs, both 1 col.
        assert_ne!(
            cursor_box_filled(),
            cursor_box_hollow(),
            "solid filled and empty space must be different glyphs"
        );
        for t in 0..32u64 {
            assert_eq!(cursor_box_glyph(t * CURSOR_BOX_BLINK_HALF_MS).width(), 1);
        }
    }

    /// Classic block pair: solid = full-cell `█` plate mate; empty = space
    /// (true empty cell, no accent plate). Same cell silhouette via on/off
    /// fill — not hole-punch `■` on green plate, not dim `█`, not short
    /// outline quads. Outer height is the terminal cell, not glyph metrics.
    #[test]
    fn cursor_box_pair_is_matching_box_rectangles() {
        // Empty half is always a plain space (classic block off).
        assert_eq!(cursor_box_hollow(), " ");
        if is_legacy_windows_console() {
            assert_eq!(cursor_box_filled(), "#");
            assert_ne!(cursor_box_filled(), cursor_box_hollow());
            return;
        }
        // Solid: █ FULL BLOCK. Empty: space (no hole-punch square).
        assert_eq!(cursor_box_filled(), "\u{2588}");
        assert_ne!(
            cursor_box_filled(),
            cursor_box_hollow(),
            "empty half must be space, not the solid full block"
        );
        // Reject empty shapes operator already turned down / hole-punch / short.
        assert_ne!(
            cursor_box_hollow(),
            "\u{2588}",
            "empty must not be FULL BLOCK (dim-of-solid / style-on-█ path)"
        );
        assert_ne!(
            cursor_box_hollow(),
            "\u{25a0}",
            "empty must not be BLACK SQUARE hole-punch (green plate + void)"
        );
        assert_ne!(
            cursor_box_hollow(),
            "\u{25af}",
            "must not be WHITE VERTICAL RECTANGLE (skinny ▯)"
        );
        assert_ne!(
            cursor_box_filled(),
            "\u{25fc}",
            "must not be BLACK MEDIUM SQUARE"
        );
        assert_ne!(
            cursor_box_hollow(),
            "\u{25fb}",
            "must not be WHITE MEDIUM SQUARE"
        );
        assert_ne!(
            cursor_box_hollow(),
            "\u{25a1}",
            "must not be tiny WHITE SQUARE"
        );
        assert_ne!(
            cursor_box_hollow(),
            "\u{2395}",
            "must not be short APL QUAD outline (mid-cell in common mono)"
        );
        assert_ne!(
            cursor_box_hollow(),
            "O",
            "empty must not be legacy ASCII hole stand-in"
        );
    }

    /// Composer caret slowly alternates solid ↔ empty (~600ms half).
    /// Glyph and phase both change: solid `█` vs empty space.
    #[test]
    fn cursor_box_blink_alternates_filled_and_hollow() {
        let half = CURSOR_BOX_BLINK_HALF_MS;
        assert!(
            (500..=800).contains(&half),
            "half-period must be slow/readable"
        );
        assert!(cursor_box_filled_phase(0));
        assert_eq!(cursor_box_glyph(0), cursor_box_filled());
        assert!(!cursor_box_filled_phase(half));
        assert_eq!(cursor_box_glyph(half), cursor_box_hollow());
        // Solid vs empty: glyphs differ across the half-period.
        assert_ne!(
            cursor_box_glyph(0),
            cursor_box_glyph(half),
            "filled and empty phases must use different glyphs"
        );
        assert_ne!(
            cursor_box_filled_phase(0),
            cursor_box_filled_phase(half),
            "filled↔empty phase must toggle with time"
        );
        assert!(cursor_box_filled_phase(half * 2));
        assert_eq!(cursor_box_glyph(half * 2), cursor_box_filled());
    }

    /// Tests drive the solid half with a pin. Wall-clock `now_ms` must not
    /// win while the pin is held, and Drop must restore timestamp phase.
    #[test]
    fn pin_cursor_box_filled_phase_overrides_timestamp() {
        let half = CURSOR_BOX_BLINK_HALF_MS;
        assert!(!cursor_box_filled_phase(half));
        {
            let _pin = pin_cursor_box_filled_phase(true);
            assert!(cursor_box_filled_phase(half));
            assert_eq!(cursor_box_glyph(half), cursor_box_filled());
        }
        assert!(!cursor_box_filled_phase(half));
        let _pin = pin_cursor_box_filled_phase(false);
        assert!(!cursor_box_filled_phase(0));
        assert_eq!(cursor_box_glyph(0), cursor_box_hollow());
    }

    // On the (non-Windows) test host the helpers must return the fancy glyphs, and the `char` helpers must agree with their `&str` siblings
    #[test]
    fn glyph_helpers_return_fancy_on_non_legacy() {
        let _pin = crate::theme::cache::pin_theme();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);
        assert!(!is_legacy_windows_console());
        assert_eq!(diamond_filled(), "\u{25C6}");
        assert_eq!(diamond_hollow(), "\u{25C7}");
        assert_eq!(diamond_dotted(), "\u{25C8}");
        assert_eq!(diamond_filled_char(), '\u{25C6}');
        assert_eq!(diamond_hollow_char(), '\u{25C7}');
        assert_eq!(braille_spinner_frames(), doge_striped_down_frames());
        assert_eq!(dot_spinner_frames().get(2).copied(), Some("\u{2e2c}"));
        assert_eq!(
            monitor_icon_frames(),
            ["\u{25CB}", "\u{25CE}", "\u{25C9}", "\u{25CE}"]
        );
    }

    // Both variants of each pre-composed button must keep a fixed column width so the right-aligned chrome lands in the same cells everywhere
    #[test]
    fn button_variants_have_stable_width() {
        for (fancy, fallback, cols) in [
            ("[\u{2717}]", "[x]", 3), // ballot_x_button
            ("[\u{2197}]", "[o]", 3), // enlarge_button
            ("[\u{2212}]", "[-]", 3), // clear_finished_button
        ] {
            assert_eq!(fancy.width(), cols, "button {fancy:?} must be {cols} cols");
            assert_eq!(
                fallback.width(),
                cols,
                "fallback {fallback:?} must be {cols} cols"
            );
        }
    }

    // The toast scrubber maps every chrome glyph that is tofu on legacy consoles to a 1-column stand-in and leaves all other text untouched
    #[test]
    fn to_legacy_glyphs_maps_known_glyphs() {
        assert_eq!(to_legacy_glyphs("\u{2713}\u{2717}\u{26A0}"), "\u{221A}x!");
        assert_eq!(
            to_legacy_glyphs("\u{2713} Saved: on"),
            "\u{221A} Saved: on",
            "only the glyph is replaced; surrounding text is preserved"
        );
        // Glyphs this module doesn't own (em dash, CJK) pass through verbatim.
        assert_eq!(
            to_legacy_glyphs("a \u{2014} \u{4e2d}"),
            "a \u{2014} \u{4e2d}"
        );
    }

    // On the (non-Windows) test host the funnel must be a zero-copy borrow so non-legacy toasts are byte-identical to the input
    #[test]
    fn legacy_glyph_fallback_is_borrow_on_non_legacy() {
        assert!(!is_legacy_windows_console());
        assert!(matches!(
            legacy_glyph_fallback("\u{2713} Saved"),
            Cow::Borrowed("\u{2713} Saved")
        ));
    }

    #[test]
    fn sanitize_toast_message_borrows_when_clean() {
        assert!(!is_legacy_windows_console());
        assert!(matches!(
            sanitize_toast_message("plain toast"),
            Cow::Borrowed("plain toast")
        ));
    }

    #[test]
    fn sanitize_toast_message_maps_controls_to_spaces() {
        let out = sanitize_toast_message("a\nb\tc");
        assert_eq!(out.as_ref(), "a b c");
        assert!(!out.chars().any(char::is_control));
    }

    #[test]
    fn forced_legacy_console_override_parses_known_values() {
        assert_eq!(parse_forced_legacy_console(Some("1")), Some(true));
        assert_eq!(parse_forced_legacy_console(Some("true")), Some(true));
        assert_eq!(parse_forced_legacy_console(Some("0")), Some(false));
        assert_eq!(parse_forced_legacy_console(Some("false")), Some(false));
        // Unset or unrecognized values defer to normal host/brand detection
        assert_eq!(parse_forced_legacy_console(None), None);
        assert_eq!(parse_forced_legacy_console(Some("")), None);
        assert_eq!(parse_forced_legacy_console(Some("yes")), None);
    }

    #[test]
    fn non_windows_is_never_legacy() {
        for brand in [
            TerminalName::Unknown,
            TerminalName::AppleTerminal,
            TerminalName::Vte,
            TerminalName::WindowsTerminal,
        ] {
            assert!(!decide_legacy_windows_console(HostOs::Macos, brand));
            assert!(!decide_legacy_windows_console(HostOs::Linux, brand));
            assert!(!decide_legacy_windows_console(HostOs::Other, brand));
        }
    }

    #[test]
    fn windows_unknown_is_legacy() {
        // Realistic ConHost case: no terminal env vars set.
        assert!(decide_legacy_windows_console(
            HostOs::Windows,
            TerminalName::Unknown
        ));
    }

    #[test]
    fn windows_terminal_is_not_legacy() {
        assert!(!decide_legacy_windows_console(
            HostOs::Windows,
            TerminalName::WindowsTerminal
        ));
    }

    #[test]
    fn vscode_family_on_windows_is_not_legacy() {
        for brand in [
            TerminalName::VsCode,
            TerminalName::Cursor,
            TerminalName::Windsurf,
            TerminalName::Zed,
        ] {
            assert!(!decide_legacy_windows_console(HostOs::Windows, brand));
        }
    }

    #[test]
    fn modern_emulators_on_windows_are_not_legacy() {
        for brand in [
            TerminalName::WezTerm,
            TerminalName::Kitty,
            TerminalName::Alacritty,
            TerminalName::Ghostty,
            TerminalName::Rio,
            TerminalName::GrokDesktop,
        ] {
            assert!(!decide_legacy_windows_console(HostOs::Windows, brand));
        }
    }

    // AppleTerminal/VTE can't actually be probed on Windows; the assertion is the default-deny safety net for unfamiliar brands
    #[test]
    fn unfamiliar_brands_on_windows_default_to_legacy() {
        for brand in [
            TerminalName::AppleTerminal,
            TerminalName::Vte,
            TerminalName::Iterm2,
            TerminalName::WarpTerminal,
        ] {
            assert!(decide_legacy_windows_console(HostOs::Windows, brand));
        }
    }
}
