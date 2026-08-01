//! Capture the current TUI frame as a PNG image.
//!
//! Plain-language contract: turn the last drawn ratatui cell buffer into a
//! PNG file the operator (or agent) can open, attach, or paste into plan
//! feedback. This is **not** an OS screenshot of other apps — only the
//! pager's own rendered frame.
//!
//! Pixel size per cell matches the gboom / kitty overlay convention
//! (8×16) so aspect is familiar on common terminals.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use ratatui::buffer::Buffer;
use ratatui::style::Color;

use crate::render::color::resolve_to_rgb;

/// Pixel width of one terminal cell in the screenshot.
pub const CELL_PX_W: u32 = 8;
/// Pixel height of one terminal cell in the screenshot.
pub const CELL_PX_H: u32 = 16;

/// Default subdirectory under `$GROK_HOME` for captured frames.
pub const SCREENSHOTS_DIR_NAME: &str = "screenshots";

/// PNG magic header (`\x89PNG\r\n\x1a\n`).
pub const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Errors from encoding or writing a TUI screenshot.
#[derive(Debug)]
pub enum TuiScreenshotError {
    EmptyBuffer,
    Encode(String),
    Io(std::io::Error),
}

impl std::fmt::Display for TuiScreenshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyBuffer => write!(f, "terminal buffer is empty (no cells to capture)"),
            Self::Encode(msg) => write!(f, "PNG encode failed: {msg}"),
            Self::Io(err) => write!(f, "screenshot write failed: {err}"),
        }
    }
}

impl std::error::Error for TuiScreenshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for TuiScreenshotError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Encode a ratatui [`Buffer`] as PNG bytes (RGB8).
///
/// Each cell becomes a `CELL_PX_W` × `CELL_PX_H` block: background fill,
/// then a simple foreground ink mark for non-whitespace glyphs so layout
/// and color remain readable without embedding a full font.
pub fn encode_ratatui_buffer_as_png(buffer: &Buffer) -> Result<Vec<u8>, TuiScreenshotError> {
    let area = buffer.area;
    if area.width == 0 || area.height == 0 {
        return Err(TuiScreenshotError::EmptyBuffer);
    }

    let img_w = area.width as u32 * CELL_PX_W;
    let img_h = area.height as u32 * CELL_PX_H;
    let mut pixels = vec![0u8; (img_w * img_h * 3) as usize];

    let default_bg = (10, 10, 10);
    let default_fg = (220, 220, 220);

    for row in 0..area.height {
        for col in 0..area.width {
            // Buffer index uses absolute terminal coords (area origin + cell).
            let cell = &buffer[(area.x + col, area.y + row)];
            let bg = resolve_to_rgb(cell.bg).unwrap_or(default_bg);
            let fg = resolve_to_rgb(cell.fg).unwrap_or(default_fg);
            // Treat Reset-as-default when both are Reset (common empty cell).
            let (bg, fg) = match (cell.bg, cell.fg) {
                (Color::Reset, Color::Reset) => (default_bg, default_fg),
                (Color::Reset, _) => (default_bg, fg),
                (_, Color::Reset) => (bg, default_fg),
                _ => (bg, fg),
            };

            let origin_x = col as u32 * CELL_PX_W;
            let origin_y = row as u32 * CELL_PX_H;
            fill_rect(
                &mut pixels,
                img_w,
                origin_x,
                origin_y,
                CELL_PX_W,
                CELL_PX_H,
                bg,
            );

            let symbol = cell.symbol();
            if has_visible_ink(symbol) {
                paint_glyph_ink(
                    &mut pixels,
                    img_w,
                    origin_x,
                    origin_y,
                    symbol.chars().next().unwrap_or('?'),
                    fg,
                );
            }
        }
    }

    encode_rgb8_png(&pixels, img_w, img_h)
}

/// Write PNG bytes to `path`, creating parent directories as needed.
pub fn write_png_bytes(path: &Path, png: &[u8]) -> Result<(), TuiScreenshotError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, png)?;
    Ok(())
}

/// Encode `buffer` and write it to `path`. Returns the path on success.
pub fn capture_buffer_to_png_file(
    buffer: &Buffer,
    path: &Path,
) -> Result<PathBuf, TuiScreenshotError> {
    let png = encode_ratatui_buffer_as_png(buffer)?;
    write_png_bytes(path, &png)?;
    Ok(path.to_path_buf())
}

/// Default destination: `{base}/screenshots/tui-{unix_secs}-{nanos}.png`.
///
/// `base` is typically `$GROK_HOME`. Callers may pass a session directory
/// instead when they want screenshots next to session artifacts.
pub fn default_screenshot_path(base: &Path) -> PathBuf {
    let (secs, nanos) = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => (d.as_secs(), d.subsec_nanos()),
        Err(_) => (0, 0),
    };
    base.join(SCREENSHOTS_DIR_NAME)
        .join(format!("tui-{secs}-{nanos:09}.png"))
}

fn encode_rgb8_png(pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>, TuiScreenshotError> {
    let mut out = Vec::new();
    let encoder =
        PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Adaptive);
    encoder
        .write_image(pixels, w, h, ExtendedColorType::Rgb8)
        .map_err(|e| TuiScreenshotError::Encode(e.to_string()))?;
    Ok(out)
}

fn fill_rect(pixels: &mut [u8], img_w: u32, x0: u32, y0: u32, w: u32, h: u32, rgb: (u8, u8, u8)) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            put_pixel(pixels, img_w, x, y, rgb);
        }
    }
}

fn put_pixel(pixels: &mut [u8], img_w: u32, x: u32, y: u32, rgb: (u8, u8, u8)) {
    let i = ((y * img_w + x) * 3) as usize;
    if i + 2 < pixels.len() {
        pixels[i] = rgb.0;
        pixels[i + 1] = rgb.1;
        pixels[i + 2] = rgb.2;
    }
}

fn has_visible_ink(symbol: &str) -> bool {
    let trimmed = symbol.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Ratatui wide-char continuation cells use a special empty/skip marker.
    if symbol == "\u{0000}" || symbol.chars().all(|c| c == ' ' || c == '\t') {
        return false;
    }
    true
}

/// Paint a simple ink mark for `ch` inside an 8×16 cell (readable layout,
/// not a full monospace font).
fn paint_glyph_ink(
    pixels: &mut [u8],
    img_w: u32,
    origin_x: u32,
    origin_y: u32,
    ch: char,
    fg: (u8, u8, u8),
) {
    // 5×7 bitmap, MSB-left in the low 5 bits; scaled ×1 inside the cell with
    // 1px left/top padding. Unknown glyphs get a solid mid-block.
    let rows = glyph_5x7(ch);
    let scale = 1u32;
    let pad_x = 1u32;
    let pad_y = 4u32;
    for (row, bits) in rows.iter().enumerate() {
        for col in 0..5u32 {
            if bits & (1 << (4 - col)) != 0 {
                let x = origin_x + pad_x + col * scale;
                let y = origin_y + pad_y + row as u32 * scale;
                put_pixel(pixels, img_w, x, y, fg);
            }
        }
    }
}

/// Tiny 5×7 font for common printable ASCII; others get a filled block.
fn glyph_5x7(ch: char) -> [u8; 7] {
    match ch {
        ' ' => [0; 7],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '#' => [0x0A, 0x1F, 0x0A, 0x0A, 0x1F, 0x0A, 0x0A],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1F],
        '3' => [0x1F, 0x01, 0x02, 0x06, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        ':' => [0x00, 0x04, 0x04, 0x00, 0x04, 0x04, 0x00],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        'A' | 'a' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' | 'b' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' | 'c' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' | 'd' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' | 'e' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' | 'f' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' | 'g' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' | 'h' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' | 'i' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' | 'j' => [0x01, 0x01, 0x01, 0x01, 0x11, 0x11, 0x0E],
        'K' | 'k' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' | 'l' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' | 'm' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' | 'n' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' | 'o' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' | 'p' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' | 'q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' | 'r' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' | 's' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' | 't' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' | 'u' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' | 'v' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' | 'w' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' | 'x' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' | 'y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' | 'z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        // Default: small filled block so unknown glyphs still show as ink.
        _ => [0x00, 0x0E, 0x0E, 0x0E, 0x0E, 0x0E, 0x00],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    fn tiny_buffer_with_text() -> Buffer {
        let area = Rect::new(0, 0, 4, 2);
        let mut buf = Buffer::empty(area);
        buf.set_string(
            0,
            0,
            "Hi",
            Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .bg(Color::Rgb(0, 0, 0)),
        );
        buf.set_string(
            0,
            1,
            "OK",
            Style::default()
                .fg(Color::Rgb(0, 255, 0))
                .bg(Color::Rgb(32, 32, 32)),
        );
        buf
    }

    #[test]
    fn encode_ratatui_buffer_as_png_writes_png_magic_and_nonempty_bytes() {
        let buf = tiny_buffer_with_text();
        let png = encode_ratatui_buffer_as_png(&buf).expect("encode");
        assert!(
            png.starts_with(PNG_MAGIC),
            "PNG must start with magic bytes, got first {:?}",
            &png[..png.len().min(8)]
        );
        assert!(
            png.len() > 32,
            "PNG payload should be more than just a header, got {} bytes",
            png.len()
        );
    }

    #[test]
    fn encode_empty_buffer_errors_without_panic() {
        let buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let err = encode_ratatui_buffer_as_png(&buf).expect_err("empty must fail");
        assert!(matches!(err, TuiScreenshotError::EmptyBuffer));
    }

    #[test]
    fn capture_buffer_to_png_file_writes_path_with_magic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("frame.png");
        let buf = tiny_buffer_with_text();
        let written = capture_buffer_to_png_file(&buf, &path).expect("write");
        assert_eq!(written, path);
        let bytes = std::fs::read(&path).expect("read back");
        assert!(bytes.starts_with(PNG_MAGIC));
    }

    #[test]
    fn default_screenshot_path_lands_under_screenshots_dir() {
        let base = PathBuf::from("/tmp/fake-grok-home");
        let path = default_screenshot_path(&base);
        assert_eq!(
            path.parent()
                .map(|p| p.file_name().and_then(|n| n.to_str())),
            Some(Some(SCREENSHOTS_DIR_NAME))
        );
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        assert!(
            name.starts_with("tui-") && name.ends_with(".png"),
            "unexpected name: {name}"
        );
    }
}
